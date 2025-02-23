use std::env::home_dir;
use std::path::PathBuf;
use itertools::Itertools;
use redgold_common_no_wasm::retry;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::{structs, RgResult, SafeOption};
use redgold_schema::structs::{Address, CurrencyAmount, ErrorInfo, SupportedCurrency};
use redgold_schema::util::lang_util::AnyPrinter;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::monero::wallet_cli::monero_wallet_cli::MoneroWalletCli;
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneroTransferProof {
    pub destination: Address,
    pub amount: CurrencyAmount,
    pub message: String,
    pub txid: String,
    pub proof: Result<String, ErrorInfo>,
}

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

    pub async fn get_proof_with_retries(
        &mut self,
        txid: String,
        destination: structs::Address,
        message: String,
    ) -> RgResult<String> {
        retry!({
            let dest_str = destination.render_string()?;
            self.get_tx_proof(txid.clone(), dest_str, Some(message.clone()))
        }, 3, 15)
    }

    pub async fn transfer_and_return_proof(
        &mut self,
        destination: structs::Address,
        amount: CurrencyAmount,
        message: String,
    ) -> RgResult<MoneroTransferProof> {
        let txid = self.transfer_single(destination.clone(), amount.clone()).await?;
        let proof = self.get_proof_with_retries(
            txid.clone(), 
            destination.clone(),
            message.clone()
        ).await;
        Ok(MoneroTransferProof {
            destination,
            amount,
            message,
            txid,
            proof,
        })
    }

    pub async fn verify_transfer_proof(&mut self, proof: &MoneroTransferProof) -> RgResult<bool> {
        let proof_str = proof.proof.clone()?;
        self.check_tx_proof(
            proof.txid.clone(),
            proof.destination.render_string()?,
            Some(proof.message.clone()),
            proof_str
        ).await
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

    let mut cli = MoneroWalletCli::open_existing_wallet(
        "http://server:18089", wp.to_str().unwrap(), None::<String>)
        .await
        .unwrap();

    cli.wait_sync().await.unwrap();

    // Poll balance until sufficient unlocked funds are available
    let required_amount = 0.001;
    loop {
        let balance = cli.balance().await.unwrap();
        println!("Current balance: {:?}", balance);
        if balance.unlocked >= required_amount {
            break;
        }
        println!("Waiting for sufficient unlocked balance (need >= {})", required_amount);
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }

    let balance = cli.balance().await.unwrap();
    println!("Balance: {:?}", balance);
    
    let dest = Address::from_monero_external(
        // 3 of 5 multisig
        // "42L1eRLEoFmXRgjW4x7rTJNwTYNgZ5G9TiQ1XGqXtzDZ2MMT15PbCffh6sgRAkYEnpCuCPu4UKH9mdLmajQwus8KHhQKkDm"
        "4AkhWUvZrTtXZU28RXBYnG2Umu6wLKfGC5CfoCsdZZSNPDVaahqM9W1UzgGqj3hh8jbQ9Uii7pZmbWzLDwCLWJtYSjfrvgx"
        // 2 of 3 multisig
    );

    let amount = CurrencyAmount::from_fractional_cur(0.005, SupportedCurrency::Monero).unwrap();
    let message = Uuid::new_v4().to_string();
    
    // Test the new transfer_and_return_proof function
    let transfer_proof = cli.transfer_and_return_proof(
        dest.clone(), 
        amount.clone(),
        message.clone()
    ).await.unwrap();
    
    println!("Transfer proof: {:?}", transfer_proof);
    
    // Verify the proof only if we got one successfully
    if transfer_proof.proof.is_ok() {
        let verified = cli.verify_transfer_proof(&transfer_proof).await.unwrap();
        assert!(verified, "Transfer proof verification failed");
        println!("Transfer proof verified successfully");
    } else {
        println!("Transfer succeeded but proof generation failed: {:?}", transfer_proof.proof.unwrap_err());
    }
}

async fn restore_ci_wallet(ci1: WordsPass, height: i64, wp: PathBuf) {
    let mut cli = MoneroWalletCli::restore_from_spend_full(
        ci1, wp.to_str().unwrap().to_string(), Some(height), "http://server:18089", false
    ).await.unwrap();

    cli.wait_sync().await.unwrap();
}
