use std::env;
use std::env::{current_dir, home_dir};
use std::io::{Read, Write};
use std::path::PathBuf;
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
use redgold_common::flume_send_help::SendErrorInfo;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_common_no_wasm::retry;

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


    fn from_pty(
        cmd: CommandBuilder,
        addr: String,
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
                daemon_address: addr,
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
            println!("Deleting old wallet files in {}", wallet_path.clone());
            let mut pb = PathBuf::from(&wallet_path);
            let mut pb2 = pb.clone();
            let pb_keys = pb2.with_extension("keys");
            println!("Deleting wallet files: {} and {}", pb.display(), pb_keys.display());
            pb.delete_file().await?;
            pb_keys.delete_file().await?;
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
        cli.try_read_expect("Do you want to do it now?").await?;
        cli.write("No").await?;
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


    pub async fn restore_from_spend_prepare_multisig(
        words: WordsPass,
        wallet_path: impl Into<String>,
        restore_height: Option<i64>,
        daemon_address: impl Into<String>,
        allow_delete_old: bool,
    ) -> RgResult<(Self, String)> {

        let mut cli = Self::restore_from_spend_full(words, wallet_path, restore_height, daemon_address, allow_delete_old).await?;

        cli.write("set enable-multisig-experimental 1").await?;
        cli.try_read_expect("Wallet password:").await?;
        cli.write("").await?;
        cli.expect_wallet().await?;


        cli.write("set").await?;
        cli.time(10);
        cli.try_read_expect("enable-multisig-experimental = 1").await?;
        cli.time(2);
        // cli.wait_sync().await?;
        cli.time(6);
        cli.write("prepare_multisig").await?;
        let out = cli.try_read_expect(
            "This includes the PRIVATE view key, so needs to be disclosed only to that multisig wallet's participants")
            .await?;

        /*
        Example using generated wallet
        --------------------------------------------------------------------------------
        MultisigxV2R1TgViniqUsZqjpEk4tkZUbmZax1Q87igiCXTPgYRQGc1LhSHoLMWF6zm7K82eNgwCdUcixV3i6oJJxcYjkhL7V6WoN7kHFNhBckcgNMMkXbm8sfhKwuZE7yQa8HCPAcG6j8ebjjH79hVMeQBWL6q5wRUETwJiUrXkhhe3dg3PKopZE11b
        Send this multisig info to all other participants, then use make_multisig <threshold> <info1> [<info2>...] with others' multisig info
        This includes the PRIVATE view key, so needs to be disclosed only to that multisig wallet's participants
         */
        cli.time(2);
        let multisig_prepared = Self::split_extract_multisig(out)?;
        // Self::wait_sync(&mut cli).await?;
        Ok((cli, multisig_prepared))
    }

    pub fn command_base(addr: &String) -> RgResult<CommandBuilder> {
        // Get current working directory and set it for the child process
        let current_dir = env::current_dir()
            .map_err(|e| "failed to get current directory".to_error_info().enhance(e.to_string()))?;
        println!("Current dir: {:?}", current_dir);
        let mut cmd = CommandBuilder::new("monero-wallet-cli");
        cmd.cwd(current_dir);
        cmd.arg("--daemon-address");
        cmd.arg(addr.clone());
        cmd.arg("--trusted-daemon");
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

    pub async fn make_multisig(&mut self, threshold: i64, peer_strings: Vec<String>) -> RgResult<String> {
        let cmd = format!("make_multisig {} {}", threshold, peer_strings.join(" "));
        self.write(cmd).await?;
        self.time(2);
        self.password().await?;
        self.time(6);
        let out = self.try_read().await?;
        let ms = Self::split_extract_multisig(out)?;
        // self.wait_sync().await?;
        Ok(ms)
    }

    /*
    Another step is needed
MultisigxV2Rn1WCSNqbsjuTXaPVfFsk3ekFF444yFN5PMCXcQHv1Pv794ZdkDZRnfVGgeP5JwpysR3ingQtQMMnmQDEXnP4qgdnh3SU2NXvfe7kMaSxMafTdPn48ko52e8UHvA4kWwpuPidBYg5JdJwdEAh8Ud7kBFX34zP33ZBbrYXcQbQKTcM3XQ8AEP8bVXHVqQSGzkAkjZRp3H63k6ZSXSYdH9WaC9pdr9FV3tx
Send this multisig info to all other participants, then use exchange_multisig_keys <info1> [<info2>...] with others' multisig info

Multisig wallet has been successfully created. Current wallet type: 2/3
Multisig address: 56MD1L4zky3bFXDQb9qvSx7PDbg8F4x1HgPrFNrDnGnYDqFZcWGswWc1p2moFa1F44ccJniY9Wkzk6urkJbEDvubHqYtkcs

     */
    pub async fn exchange_multisig_keys(&mut self, peer_strings: Vec<String>) -> RgResult<(String, bool)> {
        let cmd = format!("exchange_multisig_keys {}", peer_strings.join(" "));
        self.write(cmd).await?;
        self.time(2);
        self.password().await?;
        self.time(10);
        let all = self.try_read().await?;
        println!("All: {}", all.clone());
        let more_rounds = all.contains("Another step is needed");
        let result = if more_rounds {
            Self::split_extract_multisig(all)?
        } else {
            let split = all.split("Multisig address: ").collect_vec();
            split.get(1).ok_msg("Missing multisig address")?.trim().replace("\n", "").to_string()
        };
        // self.wait_sync().await?;

        Ok((result, more_rounds))
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


pub async fn test_wallet(words: WordsPass, id: usize) -> tokio::task::JoinHandle<RgResult<(MoneroWalletCli, String)>> {
    let addr = "http://server:18089";
    let height = get_daemon_height_retry(addr).await.unwrap() - 20;
    tokio::spawn(async move {
        let home = home_dir().unwrap();
        let wallet_path = home.join(format!("test_wallet_{}", id));
        MoneroWalletCli::restore_from_spend_prepare_multisig(
            words, wallet_path.to_str().unwrap().to_string(), Some(height), addr, true
        ).await
    })
}

#[tokio::test]
async fn test_new_wallet() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }


    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    for i in 1..5 {
        let ci = ci1.hash_derive_words(&i.to_string()).unwrap();
        words.push(ci);
    }
    println!("Num peers: {}", words.len());

    let mut jhs = vec![];
    for (i, w) in words.iter().enumerate() {
        let j1 = test_wallet(w.clone(), i).await;
        jhs.push(j1);
    };

    let mut prepared = vec![];
    for j in jhs {
        let (c, p) = j.await.unwrap().unwrap();
        prepared.push((c, p));
    }

    let mut made = vec![];

    for (mut c, p) in prepared {
        let m = c.make_multisig(3, vec![]).await.unwrap();
        made.push((c, m));
    }

    let mut peer_strings = made.iter().map(|x| x.1.clone()).collect::<Vec<String>>();

    loop {
        let these_peer_strings = peer_strings.clone();
        let mut next_peer_strings = vec![];
        let mut more_rounds = false;
        for (idx, (ref mut c, _)) in made.iter_mut().enumerate() {
            let these = these_peer_strings.iter().enumerate()
                .filter(|(i, _)| *i != idx).map(|(_, x)| { x.clone()
            }).collect::<Vec<String>>();
            let (peer, more) = c.exchange_multisig_keys(these.clone()).await.unwrap();
            more_rounds = more;
            next_peer_strings.push(peer);
        }
        peer_strings = next_peer_strings;
        if !more_rounds {
            break
        }
    }

    println!("Peer strings: {:?}", peer_strings);
    assert_eq!(peer_strings.iter().unique().count(), 1);
    let addr = peer_strings.get(0).unwrap();

    println!("Final address {}", addr);

}
