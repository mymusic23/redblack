use std::env::home_dir;
use redgold_schema::{from_hex, RgResult, SafeOption};
use redgold_schema::structs::{Address, CurrencyAmount, SupportedCurrency};
use crate::{monero::wallet_cli::monero_wallet_cli::MoneroWalletCli, TestConstants};
use crate::util::mnemonic_support::MnemonicSupport;
use uuid::Uuid;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_common_no_wasm::cleanup::Cleanup;

impl MoneroWalletCli {
    pub async fn transfer_create(&mut self, dest: &str, amount: f64) -> RgResult<String> {
        let cmd = format!("transfer {} {}", dest, amount);
        self.write(cmd).await?;
        self.try_read_expect("Is this okay?  (Y/Yes/N/No):").await?;
        self.write("Yes").await?;
        let out = self.try_read_expect("Unsigned transaction(s) successfully written to file:").await?;
        let txset = out
            .split("file: ").last().ok_msg("split")?
            .split("\n").next().ok_msg("split")?.trim().to_string();
        Ok(txset)
    }

    pub async fn sign_multisig(&mut self, filename: &str) -> RgResult<String> {
        let cmd = format!("sign_multisig {}", filename);
        self.write(cmd).await?;
        self.try_read_expect("Is this okay?  (Y/Yes/N/No):").await?;
        self.write("Yes").await?;
        let out = self.try_read_expect("Transaction successfully signed to file:").await?;
        let signed = out
            .split("file: ").last().ok_msg("split")?
            .split("\n").next().ok_msg("split")?.trim().to_string();
        Ok(signed)
    }

    pub async fn submit_multisig(&mut self, filename: &str) -> RgResult<String> {
        let cmd = format!("submit_multisig {}", filename);
        self.write(cmd).await?;
        self.try_read_expect("Transaction successfully submitted, transaction").await?;
        let out = self.try_read().await?;
        let txid = out
            .split("transaction <").last().ok_msg("split")?
            .split(">").next().ok_msg("split")?.to_string();
        Ok(txid)
    }

    pub async fn export_multisig_info(&mut self, filename: Option<String>) -> RgResult<String> {
        let output_file = filename.unwrap_or_else(|| format!("multisig_info_{}", Uuid::new_v4().to_string()));
        let cmd = format!("export_multisig_info {}", output_file);
        self.write(cmd).await?;
        self.try_read_expect("Multisig info exported to").await?;
        
        let contents = output_file.read_bytes().await?;
        Ok(hex::encode(contents))
    }

    pub async fn import_multisig_info(&mut self, info_contents: Vec<String>) -> RgResult<()> {
        let tmp_files: Vec<String> = info_contents.iter()
        .map(|_| format!("multisig_info_{}", Uuid::new_v4().to_string())).collect();
        
        // Create cleanup handlers for all temporary files
        let _cleanups: Vec<_> = tmp_files.iter()
            .map(|f| Cleanup(f.clone()))
            .collect();
        
        // Write contents to temporary files
        for (content, tmp_file) in info_contents.iter().zip(tmp_files.iter()) {
            // tmp_file.write_string(content).await?;
            tmp_file.write_bytes(from_hex(content.clone())?).await?;
        }
        
        // Import all files
        let cmd = format!("import_multisig_info {}", tmp_files.join(" "));
        self.write(cmd).await?;
        self.try_read_expect("Multisig info imported").await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_multisig_transfer() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let dest_str = "46AYBkASoYPENtzG1A6fpVQooVkxoXJokJuz1MZzMzVK4XfhULjDEVB8UGpfHhFpgXBkBbUeRdKEZJArLJqR3ZF3UNyJYFr";
    let dest = Address::from_monero_external(dest_str);
    let dest_amount = CurrencyAmount::from_fractional_cur(0.001, SupportedCurrency::Monero).unwrap();

    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    for i in 1..5 {
        let ci = ci1.hash_derive_words(&i.to_string()).unwrap();
        words.push(ci);
    }
    let addr = "http://server:18089";

    // Open all five multisig wallets
    let mut clis = Vec::new();
    for (i, _) in words.iter().enumerate() {
        let path = home_dir().unwrap().join(format!("test_wallet_{}", i));
        let cli = MoneroWalletCli::open_existing_wallet(
            addr,
            path.to_str().unwrap(),
            None::<String>
        ).await.unwrap();
        clis.push(cli);
    }

    for cli in clis.iter_mut() {
        cli.wait_sync().await.unwrap();
    }

    // First, synchronize multisig info between all participants
    println!("Synchronizing multisig info between participants...");
    let mut export_files = Vec::new();
    
    // Each wallet exports their multisig info
    for (i, cli) in clis.iter_mut().enumerate() {
        // Ensure wallet is synced before exporting
        cli.wait_sync().await.unwrap();
        let export_info = cli.export_multisig_info(None).await.unwrap();
        export_files.push(export_info);
        println!("Wallet {} exported multisig info", i + 1);
        // Small delay between exports to avoid potential race conditions
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Each wallet imports everyone else's multisig info
    for (i, cli) in clis.iter_mut().enumerate() {
        // Ensure wallet is synced before importing
        cli.wait_sync().await.unwrap();
        let other_exports: Vec<String> = export_files.iter().enumerate()
            .filter(|(j, _)| *j != i)  // Don't import your own export
            .map(|(_, export)| export.clone())
            .collect();
        cli.import_multisig_info(other_exports).await.unwrap();
        println!("Wallet {} imported info from other wallets", i + 1);
        // Small delay between imports to avoid potential race conditions
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Now create and sign the transaction
    println!("\nCreating and signing transaction...");
    
    // Create unsigned transaction with the first wallet
    let unsigned_txset = clis[0].transfer_create(dest_str, dest_amount.to_fractional()).await.unwrap();
    println!("Created unsigned transaction set: {}", unsigned_txset);

    // Get signatures from first three wallets (3-of-5 required)
    let mut current_txset = unsigned_txset;
    for i in 0..3 {
        let signed_txset = clis[i].sign_multisig(&current_txset).await.unwrap();
        println!("Wallet {} signed transaction set: {}", i + 1, signed_txset);
        current_txset = signed_txset;  // Use this signed txset for the next signer
    }

    // Submit the fully signed transaction (now has 3 of 5 signatures)
    let txid = clis[0].submit_multisig(&current_txset).await.unwrap();
    println!("Transaction submitted with ID: {}", txid);
}
