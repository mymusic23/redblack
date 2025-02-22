use redgold_common_no_wasm::ssh_like::SSHProcessInvoke;
use redgold_schema::conf::node_config::NodeConfig;
use redgold_schema::structs;
use redgold_schema::structs::{CurrencyAmount, NetworkEnvironment, SupportedCurrency};
use crate::monero::node_wrapper::{rpcs, MoneroNodeRpcInterfaceWrapper, MoneroWalletMultisigRpcState};
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;
use crate::word_pass_support::WordsPassNodeConfig;

// #[ignore]
#[tokio::test]
async fn local_three_node() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let ci = TestConstants::test_words_pass().unwrap();
    let ci1 = ci.hash_derive_words("1").unwrap();
    let ci2 = ci.hash_derive_words("2").unwrap();
    let path = TestConstants::dev_ci_kp_path();
    let pkh = ci.private_at(path.clone()).unwrap();
    let pkh1 = ci1.private_at(path.clone()).unwrap();
    let pkh2 = ci2.private_at(path.clone()).unwrap();
    let net = NetworkEnvironment::Main;
    // let addr = ci.public_at(path.clone()).unwrap().to_monero_address_from_monero_public_format(&net).unwrap();
    // let addr1 = ci1.public_at(path.clone()).unwrap().to_monero_address_from_monero_public_format(&net).unwrap();
    // let addr2 = ci2.public_at(path.clone()).unwrap().to_monero_address_from_monero_public_format(&net).unwrap();

    // temp testing only
    let mut s = SSHProcessInvoke::new("server", None);
    let user = std::env::var("USER").unwrap();
    s.user = Some(user.clone());

    let mut one = NodeConfig::from_test_id(&(1 as u16));
    let mut two = NodeConfig::from_test_id(&(2 as u16));
    let mut three = NodeConfig::from_test_id(&(3 as u16));
    let mut four = NodeConfig::from_test_id(&(4 as u16));

    one.set_words(ci.words.clone());
    two.set_words(ci1.words.clone());
    three.set_words(ci2.words.clone());
    // let words_25 = std::env::var("MONERO_HOT_SEED").unwrap();
    // let words_24 = words_25.split(" ").take(24).collect::<Vec<&str>>().join(" ");
    four.set_words(ci.words.clone());
    // four.set_words(words_24);
    one.network = NetworkEnvironment::Main;
    two.network = NetworkEnvironment::Main;
    three.network = NetworkEnvironment::Main;
    four.network = NetworkEnvironment::Main;

    one.set_rpcs(rpcs(1));
    two.set_rpcs(rpcs(2));
    three.set_rpcs(rpcs(3));
    four.set_rpcs(rpcs(4));

    let wallet_exp = r#"
#!/usr/bin/expect -f
spawn {{WALLET_CLI_PATH}} --wallet-file {{WALLET_FILENAME}} --daemon-address http://127.0.0.1:18089 --password ""
expect "Do you want to do it now*"
send "n\r"
expect "\[wallet*\]:"
send "set enable-multisig-experimental 1\r"
expect "Wallet password:"
send "\r"
expect "\[wallet*\]:"
send "save\r"
expect "\[wallet*\]:"
send "set\r"
expect "\[wallet*\]:"
send "exit\r"
expect eof
"#;

    let wallet_exp_script = wallet_exp.replace("{{WALLET_CLI_PATH}}", "monero-wallet-cli");

    let delete = false;
    let mut one_rpc = MoneroNodeRpcInterfaceWrapper::from_config(
        &one, s.clone(), "/disk/monerotw2", &wallet_exp_script, Some(delete),
    ).unwrap().unwrap();

    let mut two_rpc = MoneroNodeRpcInterfaceWrapper::from_config(
        &two, s.clone(), "/disk/monerotw3", &wallet_exp_script, Some(delete)).unwrap().unwrap();
    let mut three_rpc = MoneroNodeRpcInterfaceWrapper::from_config(
        &three, s.clone(), "/disk/monerotw4", &wallet_exp_script, Some(delete)).unwrap().unwrap();
    let mut four_rpc = MoneroNodeRpcInterfaceWrapper::from_config(
        &four, s.clone(), "/disk/monerow", &wallet_exp_script, Some(false)).unwrap().unwrap();

    let pub_keys = vec![
        one.public_key.clone(),
        two.public_key.clone(),
        three.public_key.clone()
    ];

    let fnm = "test_multisig_with_funds1".to_string();


    println!("Starting test on fnm {}", fnm.clone());


    let mut rpc_vecs = vec![one_rpc.clone(), two_rpc.clone(), three_rpc.clone()];

    // initial open and sync
    for (i, h) in rpc_vecs.iter_mut().enumerate() {
        h.prepare_wallet_fnm_and_set_multisig(&fnm, None, true).await.unwrap();
        let cur_height = one_rpc.refresh_sync_check_daemon_against_wallet().await.unwrap();
        println!("cur_height: {}", cur_height);
        let mut m = h.wallet_rpc.get_multisig().unwrap();
        let is_multisig = m.is_multisig().await.unwrap();
        println!("is_multisig from outer loop: {:?}", is_multisig);
        // one_rpc.wallet_rpc.close_wallet().await.unwrap();
    }

    let mut peer_strs: Vec<String> = vec![];
    let mut done = false;

    while !done {
        let mut new_peer_strs = vec![];
        let this_peer_strs = peer_strs.clone();

        for (idx, rpc) in rpc_vecs.iter_mut().enumerate() {
            //
            // let needs_filtering = match &rpc.state {
            //     MoneroWalletMultisigRpcState::Unknown => {false}
            //     MoneroWalletMultisigRpcState::Prepared(_) => {true}
            //     MoneroWalletMultisigRpcState::Made(_) => {true}
            //     MoneroWalletMultisigRpcState::Exchanged(_) => {false}
            //     MoneroWalletMultisigRpcState::Finalized(_) => {false}
            //     MoneroWalletMultisigRpcState::MultisigReadyToSend => {false}
            // };
            let needs_filtering = true;
            let rpc_peer_strs = if needs_filtering {
                this_peer_strs.iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, s)| s.clone()).collect::<Vec<String>>()
            } else {
                this_peer_strs.clone()
            };

            let ret = rpc.multisig_create_next(
                Some(rpc_peer_strs.clone()),
                Some(2),
                &fnm,
                true,
                2
            ).await.unwrap();

            println!("DONE wallet for peer {:?}", ret);
            ret.multisig_info_string().map(|ss| new_peer_strs.push(ss));
            if let MoneroWalletMultisigRpcState::MultisigReadyToSend = ret {
                done=true;
            }
        }
        peer_strs = new_peer_strs.clone();

    }


    println!("Preparing hot wallet");
    four_rpc.wallet_rpc.register_self_activate_ok(Some("hot".to_string()), None).await.unwrap();
    // four_rpc.wallet_rpc.sync_info()
    let sync_info = four_rpc.refresh_sync_check_daemon_against_wallet().await.unwrap();
    println!("sync info done: {:?}", sync_info);
    // let refresh = rpc.client.clone().wallet().refresh(None).await.expect("refresh");
    let b = four_rpc.wallet_rpc.get_balance().await.unwrap();
    println!("Balance for single hot wallet: {:?}", b);
    println!("Balance fractional: {:?}", b.to_fractional());
    println!("Address {}", four_rpc.wallet_rpc.self_address_str().unwrap());
    let get_hot_addr = four_rpc.wallet_rpc.clone().client.wallet().get_address(0, None).await.unwrap();
    println!("Address get_addr {}", get_hot_addr.address.to_string());


    let mut msig_strs: Vec<String> = vec![];
    let mut any_history_addr = "".to_string();
    let mut get_addr = "".to_string();

    for (i, h) in rpc_vecs.iter_mut().enumerate() {
        if let Some(a) = h.any_multisig_addr_history() {
            println!("any_multisig_addr_history: {}", a);
            any_history_addr = a;
        }
        if let Ok(a) = h.wallet_rpc.clone().client.wallet().get_address(0, None).await {
            println!("get_address: {:?}", a);
            get_addr = a.address.to_string();
        }
    }

    let destinations = vec![
        (structs::Address::from_monero_external(&any_history_addr),
        CurrencyAmount::from_fractional_cur(0.0012f64, SupportedCurrency::Monero).unwrap()),
        (structs::Address::from_monero_external(&get_addr),
        CurrencyAmount::from_fractional_cur(0.0012f64, SupportedCurrency::Monero).unwrap()),
    ];
    let tx = four_rpc.wallet_rpc.send(destinations).await;
    println!("Tx send result: {:?}", tx);

    loop {
        let h = rpc_vecs.get_mut(0).unwrap();
        let refreshed = h.refresh_sync_check_daemon_against_wallet().await.unwrap();
        println!("Refreshed: {:?}", refreshed);
        let bi = h.get_balance_all_info().await.unwrap();
        println!("Balance info: {:?}", bi);
        if bi.balance.as_pico() > 0 {
            break
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }

    for (i, h) in rpc_vecs.iter_mut().enumerate() {
        let info = h.export_multisig_info().await.unwrap();
        msig_strs.push(info);
    }
    for (idx, h) in rpc_vecs.iter_mut().enumerate() {
        let these = msig_strs.iter().enumerate()
            .filter(|(i, _)| *i != idx).map(|(_, s)| s.clone()).collect::<Vec<String>>();
        let info = h.import_multisig_info(these.clone()).await.unwrap();
        println!("Imported multisig info: {:?}", info);
    }

    let dest = four_rpc.wallet_rpc.self_address().unwrap();
    let amt =  CurrencyAmount::from_fractional_cur(0.004f64, SupportedCurrency::Monero).unwrap();
    let send = vec![(dest, amt)];
    let mut one = rpc_vecs.get(0).cloned().unwrap();
    let (prep, tx) = one.multisig_send_prepare_and_sign(send).await.unwrap();
    println!("Prepared: {:?}", prep);
    println!("Tx: {:?}", tx);
    let two = rpc_vecs.get(1).cloned().unwrap();
    let res = two.wallet_rpc.clone().get_multisig().unwrap()
        .sign_multisig(prep.multisig_txset.clone()).await.unwrap();
    println!("Sign: {:?}", res);

    let submit = one.wallet_rpc.get_multisig().unwrap()
        .submit_multisig(prep.multisig_txset.clone()).await.unwrap();

    println!("Submit: {:?}", submit);
}