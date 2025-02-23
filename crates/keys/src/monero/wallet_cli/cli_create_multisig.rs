use std::env::home_dir;
use itertools::Itertools;
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::{RgResult, SafeOption};
use crate::monero::wallet_cli::monero_wallet_cli::{get_daemon_height_retry, MoneroWalletCli};
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;

impl MoneroWalletCli {

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
            let addr_part = split.get(1).ok_msg("Missing multisig address")?.trim().replace("\n", "");
            let mut split = addr_part.chars()
                .take_while(|c| !c.is_control())  // Stop at first control char (ANSI escape)
                .collect::<String>();
            split
        };
        // self.wait_sync().await?;

        Ok((result, more_rounds))
    }


}

pub async fn test_wallet(words: WordsPass, id: usize, pfx: impl AsRef<str> + Send) -> tokio::task::JoinHandle<RgResult<(MoneroWalletCli, String)>> {
    let addr = "http://server:18089";
    let height = get_daemon_height_retry(addr).await.unwrap() - 20;
    let pfx = pfx.as_ref().to_string();
    tokio::spawn(async move {
        let home = home_dir().unwrap();
        let wallet_path = home.join(format!("{}_{}", pfx, id));
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

    let threshold = 4;
    let total_num_wallets = 8;
    let pfx = "test_wallet_4of8";

    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    for i in 1..total_num_wallets {
        let ci = ci1.hash_derive_words(&i.to_string()).unwrap();
        words.push(ci);
    }
    println!("Num peers: {}", words.len());

    let mut jhs = vec![];
    for (i, w) in words.iter().enumerate() {

        let j1 = test_wallet(w.clone(), i, pfx).await;
        jhs.push(j1);
    };

    let mut prepared = vec![];
    for j in jhs {
        let (c, p) = j.await.unwrap().unwrap();
        prepared.push((c, p));
    }

    let mut made = vec![];

    let prep_peer_strings = prepared.iter().map(|x| x.1.clone()).collect::<Vec<String>>();

    for (idx, (ref mut c, p)) in prepared.iter_mut().enumerate() {
        let strs = prep_peer_strings.iter().enumerate()
            .filter(|(i, _)| *i != idx).map(|(_, x)| { x.clone()
        }).collect::<Vec<String>>();

        let m = c.make_multisig(threshold, strs).await.unwrap();
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
