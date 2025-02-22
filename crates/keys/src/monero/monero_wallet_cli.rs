use std::env;
use std::env::current_dir;
use std::io::{Read, Write};
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
use log::info;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use regex::Regex;
use serde_json::Value;
use redgold_common::flume_send_help::SendErrorInfo;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;

pub struct MoneroWalletCli where Self: Send + 'static {
    reader_r: flume::Receiver<String>,
    writer_s: flume::Sender<String>,
    jh_reader: JoinHandle<()>,
    jh_writer: JoinHandle<()>,
    // pub child: Arc<Box<dyn portable_pty::Child + Send>>,
    pub daemon_address: String,
    pub timeout: Duration,
}


impl MoneroWalletCli {


    fn from_pty(
        master: Box<dyn portable_pty::MasterPty + Send>,
        child: Box<dyn portable_pty::Child + Send>,
        addr: String,
    ) -> RgResult<Self> {

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
                daemon_address: addr,
                timeout: Duration::from_secs(2),
            }
        )
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

    pub async fn restore_from_spend(
        words: WordsPass,
        wallet_path: impl Into<String>,
        restore_height: Option<i64>,
        daemon_address: impl Into<String>,
        allow_delete_old: bool,
    ) -> RgResult<(Self, String)> {
        let addr = daemon_address.into();
        let height = match restore_height {
            Some(h) => h,
            // Cannot do current height or it hits a monero built-in error
            None => get_daemon_height(addr.clone()).await? - 10,
        };
        println!("Height: {}", height);
        let wallet_path = wallet_path.into();
        if allow_delete_old {
            std::fs::remove_file(format!("{}.keys", wallet_path.clone())).ok();
            std::fs::remove_file(format!("{}", wallet_path.clone())).ok();
        }
        let kp = words.derive_monero_keys()?;
        let sp = kp.spend.to_string();
        let sv = kp.view.to_string();
        let address = words.monero_external_address(&NetworkEnvironment::Main)?;
        let addr_str = address.render_string()?;

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| "failed to open pty".to_error_info().enhance(e.to_string()))?;


        // Get current working directory and set it for the child process
        let current_dir = env::current_dir()
            .map_err(|e| "failed to get current directory".to_error_info().enhance(e.to_string()))?;
        println!("Current directory: {:?}", current_dir.clone());

        let mut cmd = CommandBuilder::new("monero-wallet-cli");
        cmd.cwd(current_dir);
        cmd.arg("--daemon-address");
        cmd.arg(addr.clone());
        cmd.arg("--trusted-daemon");
        cmd.arg("--restore-height");
        cmd.arg(height.to_string());
        cmd.arg("--generate-from-keys");
        cmd.arg(wallet_path.clone());

        let child = pair.slave.spawn_command(cmd)
            .map_err(|e| "failed to spawn pty".to_error_info().enhance(e.to_string()))?;
        let mut cli = Self::from_pty(pair.master, child, addr)?;

        println!("Waiting for wallet cli to start");
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
        cli.try_read_expect("Do you want to do it now?").await?;
        cli.write("No").await?;
        // tokio::time::sleep(Duration::from_secs(5)).await;
        //
        // tokio::time::sleep(Duration::from_secs(2)).await;
        tokio::time::sleep(Duration::from_secs(10)).await;
        cli.time(4);
        cli.expect_wallet().await?;
        cli.write("set enable-multisig-experimental 1").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        cli.write("set ask-password 0").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;
        cli.write("set inactivity-lock-timeout 0").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        cli.write("set").await?;
        cli.time(8);
        cli.try_read_expect("enable-multisig-experimental = 1").await?;
        let mut i = 0;
        loop {
            i += 1;
            if cli.out_of_sync().await? {
                println!("Out of sync, waiting");
            } else {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            if i > 2 {
                break;
            }
        }
        println!("Synced");

        cli.write("prepare_multisig").await?;
        let out = cli.try_read_expect(
            "This includes the PRIVATE view key, so needs to be disclosed only to that multisig wallet's participants")
            .await?;
        let multisig_prepared = Self::regex(r"(Multisig.*?)Send", out)?;

        Ok((cli, multisig_prepared))
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

    pub async fn make_multisig(&self, threshold: i64, peer_strings: Vec<String>) -> RgResult<String> {
        let cmd = format!("make_multisig {} {}", threshold, peer_strings.join(" "));
        self.write(cmd).await?;
        self.password().await?;
        self.try_read().await
    }

    pub async fn exchange_multisig_keys(&self, peer_strings: Vec<String>) -> RgResult<String> {
        let cmd = format!("exchange_multisig_keys {}", peer_strings.join(" "));
        self.write(cmd).await?;
        self.try_read().await
    }

    pub fn regex(pat: impl Into<String>, out: String) -> RgResult<String> {
        Regex::new(pat.into().as_str())
            .unwrap().find(out.as_str()).map(|m| m.as_str().trim().to_string())
            .ok_msg("Missing multisig info")
    }

    pub async fn expect_wallet(&self) -> RgResult<String> {
        self.try_read_expect("[wallet").await
    }

    pub async fn height(&self) -> RgResult<i64> {
        get_daemon_height(self.daemon_address.clone()).await
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

pub async fn test_wallet(words: WordsPass, id: usize) -> tokio::task::JoinHandle<RgResult<(MoneroWalletCli, String)>> {
    let addr = "http://server:18089";
    tokio::spawn(async move {
        MoneroWalletCli::restore_from_spend(
            words, format!("test_wallet_{}", id), None, addr, true
        ).await
    })
}
#[tokio::test]
async fn test_new_wallet() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let ci1 = TestConstants::test_words_pass().unwrap();
    let ci2 = ci1.hash_derive_words("1").unwrap();
    let ci3 = ci1.hash_derive_words("2").unwrap();

    let addr = "http://server:18089";
    // let height = get_daemon_height(addr).await.unwrap();
    // println!("Height: {}", height);
    let j1 = test_wallet(ci1, 1).await;
    let j2 = test_wallet(ci2, 2).await;
    let j3 = test_wallet(ci3, 3).await;

    let (c1, p1) = j1.await.unwrap().unwrap();
    let (c2, p2) = j2.await.unwrap().unwrap();
    let (c3, p3) = j3.await.unwrap().unwrap();

    let m1 = c1.make_multisig(2, vec![p2.clone(), p3.clone()]).await.unwrap();
    let m2 = c2.make_multisig(2, vec![p1.clone(), p3.clone()]).await.unwrap();
    let m3 = c3.make_multisig(2, vec![p1.clone(), p2.clone()]).await.unwrap();
    // c2.exchange_multisig_keys(vec![p1.clone(), p3.clone()]).await.unwrap();
    // println!("m1: {}", m1);
    // println!("m2: {}", m2);
    // println!("m3: {}", m3);

}
