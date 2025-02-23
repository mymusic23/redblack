use std::env::home_dir;
use std::path::PathBuf;
use itertools::Itertools;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::{structs, RgResult, SafeOption};
use redgold_schema::structs::{Address, CurrencyAmount, ErrorInfo, SupportedCurrency};
use redgold_schema::util::lang_util::AnyPrinter;
use crate::monero::wallet_cli::monero_wallet_cli::{MoneroWalletCli};
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;


impl MoneroWalletCli {


    pub async fn transfer_single(&mut self, destination: structs::Address, amount: CurrencyAmount) -> RgResult<String> {
        let dest = destination.render_string()?;
        let amount = amount.to_fractional();
        self.time(10);
        let xfer_cmd = format!("transfer {} {}", dest, amount);
        self.write(xfer_cmd).await?;
        self.try_read_expect("Is this okay?  (Y/Yes/N/No):").await?;
        self.write("Yes").await?;
        let out = self.try_read_expect("Transaction successfully submitted, transaction").await?;
        let txid = out
            .split("transaction <").last().clone().ok_msg("split")?
            .split(">").next().clone().ok_msg("split")?.to_string();
        Ok(txid)
    }
}


#[tokio::test]
async fn test_single_transfer() {
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

    let mut cli = MoneroWalletCli::open_existing_wallet("http://server:18089", wp.to_str().unwrap())
        .await
        .unwrap();

    cli.wait_sync().await.unwrap();

    //msig
    let dest = Address::from_monero_external(
        "42L1eRLEoFmXRgjW4x7rTJNwTYNgZ5G9TiQ1XGqXtzDZ2MMT15PbCffh6sgRAkYEnpCuCPu4UKH9mdLmajQwus8KHhQKkDm"
    );
    let amount = CurrencyAmount::from_fractional_cur(0.001, SupportedCurrency::Monero).unwrap();
    let out = cli.transfer_single(dest, amount).await.unwrap();
    println!("Transfer out: {}", out);

    let key = cli.get_tx_key(out.clone()).await.unwrap();
    println!("Tx key: {}", key);

}

async fn restore_ci_wallet(ci1: WordsPass, height: i64, wp: PathBuf) {
    let mut cli = MoneroWalletCli::restore_from_spend_full(
        ci1, wp.to_str().unwrap().to_string(), Some(height), "http://server:18089", false
    ).await.unwrap();

    cli.wait_sync().await.unwrap();
}
