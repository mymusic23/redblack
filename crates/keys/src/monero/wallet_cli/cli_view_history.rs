use std::env::home_dir;
use std::path::{Path, PathBuf};
use redgold_common_no_wasm::add;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_schema::{RgResult, SafeOption};
use redgold_schema::structs::NetworkEnvironment;
use crate::monero::key_derive::MoneroSeedBytes;
use crate::monero::wallet_cli::monero_wallet_cli::MoneroWalletCli;
use crate::TestConstants;
use crate::util::mnemonic_support::MnemonicSupport;

impl MoneroWalletCli {

    pub async fn restore_from_view<P: AsRef<Path>>(
        path: P,
        view_key: impl AsRef<str>,
        restore_height: u64,
        daemon_address: impl AsRef<str>,
        view_address: impl AsRef<str>,
        allow_delete_old: bool,
    ) -> RgResult<Self> {
        let wallet_path = path.as_ref();
        if allow_delete_old {
            Self::delete_wallet_files(wallet_path).await?;
        }

        let mut cmd = Self::command_base(&daemon_address, None::<String>)?;

        cmd.arg("--restore-height");
        cmd.arg(restore_height.to_string());
        cmd.arg("--generate-from-view-key");
        cmd.arg(wallet_path.clone());
        let mut cli = Self::from_pty(cmd, daemon_address)?;
        cli.try_read_expect("Standard address:").await?;
        cli.write(view_address.as_ref()).await?;
        cli.try_read_expect("Secret view key:").await?;
        cli.write(view_key.as_ref()).await?;
        cli.try_read_expect("Enter a new password for the wallet:").await?;
        cli.write("").await?;
        cli.try_read_expect("Confirm password:").await?;
        cli.write("").await?;
        cli.time(10);
        cli.try_read_expect("Do you want to do it now? (Y/Yes/N/No):").await?;
        cli.write("N").await?;
        cli.try_read().await?;
        cli.wait_sync().await?;
        Ok(cli)
    }
}



#[tokio::test]
async fn test_restore_view() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }

    let ci1 = TestConstants::test_words_pass().unwrap();
    let mut words = vec![ci1.clone()];
    for i in 1..5 {
        let ci = ci1.hash_derive_words(&i.to_string()).unwrap();
        words.push(ci);
    }


    let height = 3352263 - 10;
    let home = home_dir().unwrap();
    let wp = home.join("restore_view_hot");

    let view = ci1.derive_monero_keys().unwrap().view.to_string();
    let addr = ci1.derive_monero_address(&NetworkEnvironment::Main).unwrap().to_string();
    let mut cli = MoneroWalletCli::restore_from_view(
        wp,
        view,
        height,
        "http://server:18089",
        addr,
        true
    )
        .await
        .unwrap();

    let xfers = cli.export_transfers().await.unwrap();
    for x in xfers {
        println!("{:?}", x);
    }
    // cli.wait_sync().await.unwrap();
}