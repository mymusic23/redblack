use std::env::home_dir;
use itertools::Itertools;
use redgold_schema::errors::into_error::ToErrorInfo;
use crate::monero::wallet_cli::monero_wallet_cli::{test_wallet, MoneroWalletCli};
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;


impl MoneroWalletCli {
    // pub fn open_existing_wallet(path: AsRef<str>)

}

#[tokio::test]
async fn test_multisig_transfer() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    for i in 1..5 {
        let ci = ci1.hash_derive_words(&i.to_string()).unwrap();
        words.push(ci);
    }

    let height = 3313322;
    let home = home_dir().unwrap();
    let wp = home.join("hot");
    let mut cli = MoneroWalletCli::restore_from_spend_full(
        ci1, wp.to_str().unwrap().to_string(), Some(height), "http://server:18089", false
    ).await.unwrap();

    cli.wait_sync().await.unwrap();


    // let mut jhs = vec![];
    // for w in words {
    //     let j1 = test_wallet(w, 1).await;
    //     jhs.push(j1);
    // };

    // let mut prepared = vec![];
    // for j in jhs {
    //     let (c, p) = j.await.unwrap().unwrap();
    //     prepared.push((c, p));
    // }

    // let mut made = vec![];

    // for (mut c, p) in prepared {
    //     let m = c.make_multisig(3, vec![]).await.unwrap();
    //     made.push((c, m));
    // }

    // let mut peer_strings = made.iter().map(|x| x.1.clone()).collect::<Vec<String>>();

    // loop {
    //     let these_peer_strings = peer_strings.clone();
    //     let mut next_peer_strings = vec![];
    //     let mut more_rounds = false;
    //     for (idx, (ref mut c, _)) in made.iter_mut().enumerate() {
    //         let these = these_peer_strings.iter().enumerate()
    //             .filter(|(i, _)| *i != idx).map(|(_, x)| { x.clone()
    //         }).collect::<Vec<String>>();
    //         let (peer, more) = c.exchange_multisig_keys(these.clone()).await.unwrap();
    //         more_rounds = more;
    //         next_peer_strings.push(peer);
    //     }
    //     peer_strings = next_peer_strings;
    //     if !more_rounds {
    //         break
    //     }
    // }

    // println!("Peer strings: {:?}", peer_strings);
    // assert_eq!(peer_strings.iter().unique().count(), 1);
    // let addr = peer_strings.get(0).unwrap();

    // println!("Final address {}", addr);

}
