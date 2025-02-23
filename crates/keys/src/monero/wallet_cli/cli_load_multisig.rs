use std::env::home_dir;
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::RgResult;
use redgold_schema::util::lang_util::AnyPrinter;
use crate::monero::wallet_cli::monero_wallet_cli::{get_daemon_height_retry, MoneroWalletCli};
use crate::TestConstants;


impl MoneroWalletCli {
    
}
#[tokio::test]
async fn test_load_ms_wallet() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    let addr = "http://server:18089";

    let path = home_dir().unwrap().join("test_wallet_0");

    let cli = MoneroWalletCli::open_existing_wallet(
        addr,
        path.to_str().unwrap(),
        None::<String>
    ).await.unwrap();

    println!("Balance: {:?}", cli.balance().await.unwrap());
    // 46AYBkASoYPENtzG1A6fpVQooVkxoXJokJuz1MZzMzVK4XfhULjDEVB8UGpfHhFpgXBkBbUeRdKEZJArLJqR3ZF3UNyJYFr

    // cli.help_all().await.unwrap().print();

    // let addr = cli.address().await.unwrap();
    // println!("Address: {}", addr);

}