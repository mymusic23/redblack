use std::path::PathBuf;
use redgold_common::external_resources::PeerBroadcast;
use redgold_common::flume_send_help::SendErrorInfo;
use redgold_data::data_store::DataStore;
use redgold_keys::monero::wallet_cli::monero_wallet_cli::{get_daemon_height_retry, MoneroWalletCli};
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::observability::errors::Loggable;
use redgold_schema::{ErrorInfoContext, RgResult, SafeOption};
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_schema::util::times::current_time_millis;
use crate::services::monero::{MoneroInstanceSecretData, MoneroSyncInteraction, MoneroTransactionInfo, MoneroWalletMessage, MoneroWalletMessageType, MoneroWalletResponse, MultisigStage};
use crate::services::monero::MoneroWalletResponse::PeerCreate;

#[derive(Clone)]
pub struct LiveWallet {
    pub cli: MoneroWalletCli,
    pub stage: MultisigStage,
    pub last_self_peer_info: Option<String>,
    pub data: MoneroInstanceSecretData,
}


#[derive(Clone)]
pub struct WalletThread<B> where B: PeerBroadcast + 'static {
    pub words: WordsPass,
    pub base_wallet_working_directory: PathBuf,
    pub daemon_address: String,
    pub peer_broadcast: B,
    pub live_wallet: RgResult<LiveWallet>,
    pub is_restore: bool,
    pub data_store: DataStore,
}


impl<B> WalletThread<B> where B: PeerBroadcast + 'static {

    async fn start_new_wallet(&self, message: &MoneroSyncInteraction
    ) -> RgResult<(MoneroWalletCli, MoneroInstanceSecretData)> {
        let height = get_daemon_height_retry(&self.daemon_address).await?;
        // Creating a new wallet
        let address = message.address_descriptor_address_from_pk_threshold();
        let identifier = address.render_string()?;
        let wallet_working_dir = self.base_wallet_working_directory.join(&identifier);
        let wallet_path = wallet_working_dir.join(crate::services::monero::MULTISIG_WALLET_FILENAME);
        if tokio::fs::try_exists(&wallet_working_dir).await.error_info("Check exists")? {
            "Wallet already exists".to_error()?;
        }
        tokio::fs::create_dir_all(&wallet_working_dir).await.error_info("Failed to create dir")?;

        let cli = MoneroWalletCli::restore_from_spend_precursor(
            crate::services::monero::MULTISIG_WALLET_FILENAME,
            // Magic number due to weird cli error
            Some(height - 20),
            self.daemon_address.clone(),
            false,
            Some(wallet_working_dir.clone())
        ).await?;

        let data = MoneroInstanceSecretData {
            wallet_group_key: address.clone(),
            wallet_group_key_str: identifier,
            wallet_prefix_dir: self.base_wallet_working_directory.to_string_lossy().to_string(),
            working_dir: wallet_working_dir.clone().to_string_lossy().to_string(),
            wallet_name: crate::services::monero::MULTISIG_WALLET_FILENAME.to_string(),
            wallet_path: wallet_path.to_string_lossy().to_string(),
            wallet_data: Default::default(),
            peer_strings: vec![],
            created_restore_height: height,
            created_at: current_time_millis(),
            last_wallet_height: height,
            last_sync_time: current_time_millis(),
            raw_monero_output_address: "".to_string(),
        };
        Ok((cli, data))
    }
    pub async fn create_multisig(
        &mut self,
        message: &MoneroSyncInteraction,
    ) -> RgResult<MoneroWalletResponse> {
        let (cli, data) = self.start_new_wallet(message).await?;
        let w = LiveWallet::new(cli, data);
        self.live_wallet = Ok(w.clone());
        match self.continue_from_new_live_wallet(message).await {
            Ok(a) => {
                Ok(a)
            }
            Err(e) => {
                w.destroy().await;
                self.live_wallet = Err(e.clone());
                Err(e)
            }
        }
    }


    pub async fn message_round(
        &self,
        message: &MoneroSyncInteraction,
        input: Vec<String>,
    ) -> RgResult<Vec<String>> {
        let mut peer_output_strings = vec![];
        for r in self.peer_broadcast.broadcast(&message.peer_pks.clone(), message.request(input)).await? {
            let r = r?;
            let r = r.monero_multisig_formation_response.ok_msg("No response from peer")?;
            peer_output_strings.push(r);
        }
        Ok(peer_output_strings)
    }
    pub async fn continue_from_new_live_wallet(
        &mut self,
        message: &MoneroSyncInteraction
    ) -> RgResult<MoneroWalletResponse> {
        let mut history = vec![];
        let mut w = self.live_wallet.as_ref().clone().unwrap().clone();
        let (mut cli2, prepared) = w.cli.clone().restore_from_spend_up_to_prepare(self.words.clone()).await?;
        let peer_prepared_strs = self.message_round(&message, vec![]).await?;
        let mut peer_prepared_strs_incl_self = vec![prepared.clone()];
        peer_prepared_strs_incl_self.extend(peer_prepared_strs.clone());
        history.push(peer_prepared_strs_incl_self.clone());

        let made = cli2.make_multisig(message.threshold, peer_prepared_strs.clone()).await?;
        let peer_made_strs = self.message_round(&message, peer_prepared_strs).await?;
        let mut peer_made_strs_incl_self = vec![made.clone()];
        peer_made_strs_incl_self.extend(peer_made_strs.clone());
        history.push(peer_made_strs_incl_self.clone());

        let mut more_rounds = true;
        let mut peer_strings = peer_made_strs.clone();
        let mut peer_strings_incl_self = peer_made_strs_incl_self.clone();

        let mut raw_address = "".to_string();

        while more_rounds {
            let (exchanged, do_more) = cli2.exchange_multisig_keys(peer_strings).await?;
            more_rounds = do_more;
            let new_peer_exchange_strs = self.message_round(
                &message, peer_strings_incl_self
            ).await?;
            let mut new_incl_self = vec![exchanged.clone()];
            new_incl_self.extend(new_peer_exchange_strs.clone());

            peer_strings = new_peer_exchange_strs;
            peer_strings_incl_self = new_incl_self;
            history.push(peer_strings_incl_self.clone());
            raw_address = exchanged.clone();
        }
        w.data.wallet_data = MoneroWalletCli::get_wallet_files(
            w.data.wallet_name.clone(),
            w.data.wallet_path.clone()
        ).await?;
        w.data.peer_strings = history;
        w.data.raw_monero_output_address = raw_address.clone();
        self.live_wallet = Ok(w.clone());
        Ok(MoneroWalletResponse::InstanceCreated(w.data.clone()))
    }

    pub async fn handle_message(&mut self, message: MoneroSyncInteraction) -> RgResult<()> {
        let response = if self.live_wallet.is_err() || !self.is_restore {
            match message.message {
                MoneroWalletMessage::InternalCreateMultisigAsProposer => {
                    let ret = self.create_multisig(&message).await;
                    if let Ok(MoneroWalletResponse::InstanceCreated(d)) = &ret {
                        self.persist(d.clone()).await.ok();
                    }
                    ret
                }
                MoneroWalletMessage::HandleNextStageRequest => {
                    self.peer_initial_formation_handler(&message).await
                }
                MoneroWalletMessage::InternalGetTransactions => {"err".to_error()}
            }
        } else {
            let mut live_wallet = self.live_wallet.clone().unwrap();
            match message.message {
                MoneroWalletMessage::InternalCreateMultisigAsProposer => {
                    "legit error, shouldn't happen".to_error()
                }
                MoneroWalletMessage::HandleNextStageRequest => {
                    let ret = live_wallet.peer_handle_next_stage_request_active(&message).await;
                    if &live_wallet.stage == &MultisigStage::Ready {
                        self.persist(live_wallet.data.clone()).await.ok();
                    }
                    self.live_wallet = Ok(live_wallet);
                    ret
                }
                MoneroWalletMessage::InternalGetTransactions => {
                    live_wallet.get_transactions().await
                }
            }
        };
        message.response.send_rg_err(response)
    }
    
    pub async fn peer_initial_formation_handler(
        &mut self,
        message: &MoneroSyncInteraction,
    ) -> RgResult<MoneroWalletResponse> {
        let (cli, data) = self.start_new_wallet(message).await?;
        let mut w = LiveWallet::new(cli.clone(), data);
        match cli.restore_from_spend_up_to_prepare(self.words.clone()).await {
            Ok((_, prepared)) => {
                w.stage = MultisigStage::Prepared;
                w.last_self_peer_info = Some(prepared.clone());
                self.live_wallet = Ok(w);
                Ok(PeerCreate(prepared))
            }
            Err(e) => {
                w.destroy().await;
                self.live_wallet = Err(e.clone());
                Err(e)
            }
        }
    }

    async fn persist(&self, data: MoneroInstanceSecretData) -> RgResult<()> {
        self.data_store.config_store.set_json(
            &data.key(),
            data.json_or()
        ).await.map(|_| ())
    }
}


impl LiveWallet {

    pub fn new(wallet: MoneroWalletCli, data: MoneroInstanceSecretData) -> Self {
        Self {
            cli: wallet,
            stage: MultisigStage::Creating,
            last_self_peer_info: None,
            data
        }
    }
    pub async fn open_existing_wallet(
        daemon_address: String,
        data: MoneroInstanceSecretData
    ) -> RgResult<Self> {
        let cli = MoneroWalletCli::open_existing_wallet(
            daemon_address.clone(),
            data.wallet_path.clone(),
            Some(data.working_dir.clone()),
        ).await?;
        Ok(Self {
            cli,
            stage: MultisigStage::Ready,
            last_self_peer_info: None,
            data,
        })
    }
    pub async fn get_transactions(&self) -> RgResult<MoneroWalletResponse> {
        Ok(MoneroWalletResponse::Transactions(MoneroTransactionInfo{
            tx: self.cli.export_transfers().await?,
            last_updated: current_time_millis(),
        }))
    }

    pub fn filter_self(&self, peer_strs: Vec<String>) -> RgResult<Vec<String>> {
        let pi = self.last_self_peer_info.as_ref().ok_msg("No self peer info")?;
        Ok(peer_strs.into_iter().filter(|x| x != pi).collect())
    }

    pub async fn destroy(&self) {
        self.cli.destroy();
        self.data.wallet_data.destroy().await.ok();
    }


    pub async fn peer_handle_next_stage_request_active(
        &mut self,
        message: &MoneroSyncInteraction
    ) -> RgResult<MoneroWalletResponse> {
        let ret = match self.stage {
            MultisigStage::Creating => {
                "Wallet not prepared".to_error()?
            }
            MultisigStage::Prepared => {
                let ret = self.cli.make_multisig(
                    message.threshold,
                    self.filter_self(message.peer_strings.clone())?
                ).await?;
                self.stage = MultisigStage::Made;
                ret
            }
            MultisigStage::Made => {
                let (ret, more_rounds) = self.cli.exchange_multisig_keys(
                    self.filter_self(message.peer_strings.clone())?
                ).await?;
                if more_rounds {
                    self.stage = MultisigStage::Exchanged;
                } else {
                    self.stage = MultisigStage::Ready;
                }
                ret
            }
            MultisigStage::Exchanged => {
                let (ret, more_rounds) = self.cli.exchange_multisig_keys(
                    self.filter_self(message.peer_strings.clone())?
                ).await?;
                if more_rounds {
                    self.stage = MultisigStage::Exchanged;
                } else {
                    self.stage = MultisigStage::Ready;
                }
                ret
            }
            MultisigStage::ExportedForTransfer => {
                self.cli.import_multisig_info(self.filter_self(message.peer_strings.clone())?).await?;
                self.stage = MultisigStage::ImportedForTransfer;
                "".to_string()
            }
            MultisigStage::ImportedForTransfer => {
                let str = message.peer_strings.iter().next().ok_msg("No peer strings")?;
                let ret = self.cli.sign_multisig(str).await?;
                self.stage = MultisigStage::Ready;
                ret
            }
            MultisigStage::Ready => {
                let export = self.cli.export_multisig_info(None).await?;
                self.stage = MultisigStage::ExportedForTransfer;
                export
            }
        };
        self.last_self_peer_info = Some(ret.clone());
        Ok(PeerCreate(ret))
    }

}
