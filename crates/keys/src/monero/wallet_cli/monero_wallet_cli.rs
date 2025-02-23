use std::env;
use std::env::{current_dir, home_dir};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::ptr::write;
use std::sync::Arc;
use std::thread::JoinHandle;
use redgold_schema::keys::words_pass::WordsPass;
use tokio::process::{Command, Child, ChildStdin, ChildStdout, ChildStderr};
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt, AsyncReadExt};
use redgold_schema::{ErrorInfoContext, RgResult, SafeOption, error_info};
use redgold_schema::structs::{ErrorInfo, NetworkEnvironment};
use crate::monero::key_derive::MoneroSeedBytes;
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;
use std::time::Duration;
use itertools::Itertools;
use log::info;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use redgold_common::flume_send_help::SendErrorInfo;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_common_no_wasm::retry;
use redgold_schema::observability::errors::EnhanceErrorInfo;

pub struct MoneroWalletCli where Self: Send + 'static {
    reader_r: flume::Receiver<String>,
    writer_s: flume::Sender<String>,
    jh_reader: JoinHandle<()>,
    jh_writer: JoinHandle<()>,
    // pub child: Arc<Box<dyn portable_pty::Child + Send>>,
    pub daemon_address: String,
    pub timeout: Duration,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WalletDiskFiles{
    pub wallet_name: String,
    pub on_disk_path: String,
    pub wallet_bytes: Vec<u8>,
    pub keys_bytes: Vec<u8>,
}


impl MoneroWalletCli {

    pub async fn get_tx_key(&self, txid: impl AsRef<str>) -> RgResult<String> {
        self.write(format!("get_tx_key {}", txid.as_ref())).await?;
        self.try_read().await
    }
    pub async fn open_existing_wallet(
        daemon_address: impl AsRef<str>,
        path: impl AsRef<str>
    ) -> RgResult<MoneroWalletCli> {
        let mut cmd = Self::command_base(daemon_address.as_ref())?;
        cmd.arg("--wallet-file");
        cmd.arg(path.as_ref());
        let mut cli = Self::from_pty(cmd, daemon_address)?;
        cli.password().await?;
        cli.time(10);
        cli.expect_wallet().await?;
        Ok(cli)
    }

    pub async fn address(&self) -> RgResult<String> {
        self.write("address").await?;
        let out = self.try_read_expect("Primary address").await?;
        // println!("Primary address output here: {}", out.clone());
        let split = out.split("\n").last().ok_msg("Missing address")?;
        // println!("Split newline {}", split);
        let split = split.split_ascii_whitespace().collect_vec();
        // println!("Split whitespace {:?}", split);
        let idx = split.iter().enumerate().find(|(_, x)| **x == "Primary").ok_msg("Missing primary")?.0;
        let addr = split.get(idx-1).ok_msg("Missing address")?.trim().to_string();
        Ok(addr)
    }

    pub async fn help_all(&self) -> RgResult<String> {
        self.write("help all").await?;
        self.try_read().await
    }

    pub async fn balance(&self) -> RgResult<String> {
        self.write("balance").await?;
        self.try_read().await
    }

    pub(crate) fn from_pty(
        cmd: CommandBuilder,
        addr: impl AsRef<str>,
    ) -> RgResult<Self> {
        let pair = Self::open_pty()?;
        let master = pair.master;
        let _child = pair.slave.spawn_command(cmd)
            .map_err(|e| "failed to spawn pty".to_error_info().enhance(e.to_string()))?;

        let reader = master.try_clone_reader()
            .map_err(|e| "failed to open clone reader pty".to_error_info().enhance(e.to_string()))?;
        let writer = master.take_writer()
            .map_err(|e| "failed to open pty writer".to_error_info().enhance(e.to_string()))?;

        let (reader_s, reader_r) = flume::unbounded::<String>();
        let (writer_s, writer_r) = flume::unbounded::<String>();
        let jh_reader = std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = vec![0; 65536];
            // println!("Reading");
            loop {
                match reader.read(&mut buf).error_info("Read failure") {
                    Ok(r) => {
                        let read_str = std::str::from_utf8(&buf[..r])
                            .unwrap();
                        let read = read_str.trim();
                        if read.is_empty() {
                            continue
                        }
                        // println!("Read: {}", read);
                        reader_s.send(read.to_string()).ok();
                    }
                    Err(r) => {
                        // info!("Error: {}", r.json_or());
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        let w2 = writer_r.clone();
        let jh_writer = std::thread::spawn(move || {
            let mut writer = writer;
            let writer_r = w2;
            loop {
                let next = writer_r.try_recv();
                if let Ok(next) = next {
                    println!("Writing {}", next.clone());
                    writer.write_all(next.as_bytes()).unwrap();
                }
            }
        });

        Ok(
            Self {
                reader_r,
                writer_s,
                jh_reader,
                jh_writer,
                // child: Arc::new(child),
                daemon_address: addr.as_ref().to_string(),
                timeout: Duration::from_secs(2),
            }
        )
    }

    pub async fn get_wallet_files(wallet_name: impl Into<String>, full_path: impl Into<String>) -> RgResult<WalletDiskFiles> {
        let name = wallet_name.into();
        let path = full_path.into();
        let pb = PathBuf::from(&path);
        let pb_keys = pb.with_extension("keys");
        Ok(WalletDiskFiles {
            wallet_name: name.clone(),
            on_disk_path: path.clone(),
            wallet_bytes: path.read_bytes().await?,
            keys_bytes: pb_keys.read_bytes().await?,
        })
    }

    pub fn time(&mut self, t: u64) {
        self.timeout = Duration::from_secs(t);
    }

    pub async fn try_read(&self) -> RgResult<String> {
        let instant = tokio::time::Instant::now();
        let mut all = String::new();
        while instant.elapsed() < self.timeout {
            let result = self.reader_r.try_recv();
            if let Ok(result) = result {
                println!("Result: {}", result);
                all.push_str(&result);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(all)
    }

    pub async fn write(&self, cmd: impl Into<String>) -> RgResult<()> {
        let format = format!("{}\n", cmd.into());
        self.writer_s.send_rg_err(format)?;
        Ok(())
    }

    pub async fn try_read_expect(&self, expect: impl Into<String>) -> RgResult<String> {
        let expect = expect.into();
        let output = self.try_read().await?;
        if !output.contains(&expect) {
            format!("Expected {} but got {}", expect, output).to_error()?;
        }
        Ok(output)
    }

    pub async fn delete_wallet_files<P: AsRef<Path>>(wallet_path: P) -> RgResult<()> {
        let wallet_path = wallet_path.as_ref();
        println!("Deleting old wallet files in {}", wallet_path.to_str().unwrap().to_string().clone());
        let mut pb = PathBuf::from(&wallet_path);
        let mut pb2 = pb.clone();
        let pb_keys = pb2.with_extension("keys");
        println!("Deleting wallet files: {} and {}", pb.display(), pb_keys.display());
        pb.delete_file().await?;
        pb_keys.delete_file().await?;
        Ok(())
    }

    pub async fn restore_from_spend_precursor(
        wallet_path: impl Into<String>,
        restore_height: Option<i64>,
        daemon_address: impl Into<String>,
        allow_delete_old: bool,
    ) -> RgResult<Self> {
        let addr = daemon_address.into();

        // println!("Height: {}", height);
        let wallet_path = wallet_path.into();
        if allow_delete_old {
          Self::delete_wallet_files(&wallet_path).await?;
        }

        let height = match restore_height {
            Some(h) => h,
            // Cannot do current height or it hits a monero built-in error
            None => get_daemon_height(addr.clone()).await? - 20,
        };

        let mut cmd = Self::command_base(&addr)?;

        cmd.arg("--restore-height");
        cmd.arg(height.to_string());

        cmd.arg("--generate-from-keys");
        cmd.arg(wallet_path.clone());
        let cli = Self::from_pty(cmd, addr)?;
        Ok(cli)
    }


    pub async fn restore_from_spend_full(
        words: WordsPass,
        wallet_path: impl Into<String>,
        restore_height: Option<i64>,
        daemon_address: impl Into<String>,
        allow_delete_old: bool,
    ) -> RgResult<Self> {

        let kp = words.derive_monero_keys()?;
        let sp = kp.spend.to_string();
        let sv = kp.view.to_string();
        let address = words.monero_external_address(&NetworkEnvironment::Main)?;
        let addr_str = address.render_string()?;

        let mut cli = Self::restore_from_spend_precursor(wallet_path, restore_height, daemon_address, allow_delete_old).await?;
        cli.try_read_expect("Standard address:").await?;
        cli.write(addr_str).await?;
        cli.try_read_expect("Secret spend key:").await?;
        cli.write(sp).await?;
        cli.try_read_expect("Secret view key:").await?;
        cli.write(sv).await?;
        cli.try_read_expect("Enter a new password for the wallet:").await?;
        cli.write("").await?;
        cli.try_read_expect("Confirm password:").await?;
        cli.write("").await?;
        cli.time(10);
        // Tried this but it blows up, possibly due to some console refresh?
        // cli.try_read_expect("or alternatively from specific date (YYYY-MM-DD):").await?;
        // if untrusted use this.
        // cli.try_read_expect("Generated new wallet:").await?;
        // tokio::time::sleep(Duration::from_secs(3)).await;
        let out = cli.try_read().await?;
        if out.contains("Do you want to do it now?") {
            cli.write("No").await?;
        } else if out.contains("Still apply restore height?") {
            cli.write("Yes").await?;
            cli.try_read_expect("Do you want to do it now?").await?;
            cli.write("No").await?;
        } else {
            "Unknown prompt".to_error()
                .with_detail("output", out)?;
        }
        // tokio::time::sleep(Duration::from_secs(5)).await;
        //
        // tokio::time::sleep(Duration::from_secs(2)).await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        cli.time(2);
        cli.expect_wallet().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        cli.write("set ask-password 0").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;
        cli.write("set inactivity-lock-timeout 0").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(cli)
    }


    pub fn command_base(addr: impl AsRef<str>) -> RgResult<CommandBuilder> {
        // Get current working directory and set it for the child process
        let current_dir = env::current_dir()
            .map_err(|e| "failed to get current directory".to_error_info().enhance(e.to_string()))?;
        println!("Current dir: {:?}", current_dir);
        let string = addr.as_ref().to_string();

        let mut cmd = CommandBuilder::new("monero-wallet-cli");
        cmd.cwd(current_dir);
        if !string.is_empty() {
            cmd.arg("--daemon-address");
            cmd.arg(string);
            cmd.arg("--trusted-daemon");
        }
        Ok(cmd)
    }

    pub fn open_pty() -> Result<PtyPair, ErrorInfo> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| "failed to open pty".to_error_info().enhance(e.to_string()))?;
        Ok(pair)
    }

    pub async fn wait_sync(&mut self) -> Result<(), ErrorInfo> {
        let mut i = 0;
        let instant = tokio::time::Instant::now();
        loop {
            i += 1;
            if self.out_of_sync().await? {
                let elapsed = instant.elapsed().as_secs();
                println!("Out of sync, waiting {i}: {elapsed} seconds");
            } else {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        println!("Synced");
        Ok(())
    }

    pub fn split_extract_multisig(out: String) -> Result<String, ErrorInfo> {
        let vec = out.split("Multisig").collect::<Vec<&str>>();
        let vec = vec.get(1).ok_msg("Missing multisig info")?.clone();
        let vec = vec.split("Send").collect::<Vec<&str>>();
        let vec = vec.get(0).ok_msg("Missing multisig info")?.clone().trim().replace("\n", "");
        let multisig_prepared = format!("Multisig{}", vec);
        Ok(multisig_prepared)
    }

    pub async fn password(&self) -> RgResult<()> {
        self.try_read_expect("Wallet password:").await?;
        self.write("").await?;
        Ok(())
    }
    pub async fn out_of_sync(&self) -> RgResult<bool> {
        self.write("status").await?;
        let out = self.try_read_expect("[wallet").await?;
        Ok(out.contains("out of sync"))
    }

    pub async fn expect_wallet(&self) -> RgResult<String> {
        self.try_read_expect("[wallet").await
    }



}

// $ curl http://server:18089/get_height -H 'Content-Type: application/json'
pub async fn get_daemon_height(url: impl Into<String>) -> RgResult<i64> {
    let base_url = url.into();
    let url = format!("{}/get_height", base_url.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let res = client.get(&url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .error_info("Failed to send request")?;

    let res = res.json::<Value>()
        .await
        .error_info("Failed to parse response")?;

    let height = res.get("height").ok_msg("Missing height")?;
    let height = height.as_u64().ok_msg("Height is not a number")?;
    Ok(height as i64)
}

pub async fn get_daemon_height_retry(url: impl AsRef<str>) -> RgResult<i64> {
    retry!(get_daemon_height(url.as_ref()))
}

