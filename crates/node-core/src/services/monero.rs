use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::Join;
use redgold_keys::monero::wallet_cli::monero_wallet_cli::{get_daemon_height_retry, MoneroWalletCli, MoneroWalletDiskFiles};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use redgold_common::external_resources::PeerBroadcast;
use redgold_common::flume_send_help::SendErrorInfo;
use redgold_common_no_wasm::ssh_like::{LocalSSHLike, SSHOrCommandLike};
use redgold_common_no_wasm::stream_handlers::{IntervalFoldOrReceive, TryRecvForEach};
use redgold_keys::monero::node_wrapper::{MoneroNodeRpcInterfaceWrapper, MoneroWalletMultisigRpcState, PartySecretInstanceData};
use redgold_keys::monero::rpc_multisig::MoneroWalletRpcMultisigClient;
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_schema::observability::errors::Loggable;
use redgold_schema::{ErrorInfoContext, RgResult, SafeOption};
use redgold_schema::structs::{Address, AddressDescriptor, ErrorInfo, MoneroMultisigFormationRequest, PartyId, PublicKey, Weighting};
use redgold_schema::util::times::current_time_millis;
use tokio::time::Instant;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_keys::monero::wallet_cli::cli_tx_history::MoneroCliHistoryTransaction;
use redgold_schema::keys::words_pass::WordsPass;
use redgold_schema::message::Request;
use redgold_schema::structs::ValidationLiveness::Live;
use crate::services::monero::MoneroWalletMessageType::Formation;
use crate::services::monero::MoneroWalletResponse::PeerCreate;
use crate::services::monero_group::{LiveWallet, WalletThread};

#[derive(Clone, Debug)]
pub enum MultisigStage {
    // Creation of the multisig wallet
    Creating,
    Prepared,
    Made,
    Exchanged,
    // Transfer
    ExportedForTransfer,
    ImportedForTransfer,
    Ready
}

#[derive(Clone)]
pub struct IndividualWalletSyncHandlerThread {
    jh: Arc<JoinHandle<RgResult<()>>>,
    sender: flume::Sender<MoneroSyncInteraction>,
}


#[derive(Clone)]
pub struct MoneroWalletSyncWriter<B: PeerBroadcast + 'static> {
    pub sync_handlers: HashMap<Address, IndividualWalletSyncHandlerThread>,
    pub peer_broadcast: B,
    pub base_wallet_working_directory: PathBuf,
    pub daemon_address: String,
    pub words: WordsPass,
}


#[derive(Clone, Debug, PartialEq)]
pub enum MoneroWalletMessageType {
    Formation,
    Transfer
}

#[derive(Clone, Debug, PartialEq)]
pub enum MoneroWalletMessage {
    // This is the first step, the proposer starts the operation internally
    InternalCreateMultisigAsProposer,
    // Next, the potential party members must initialize the next step of the operation
    HandleNextStageRequest,
    // Return cached transactions (done via periodic updates)
    InternalGetTransactions
}

// This is the internal request type
#[derive(Clone, Debug)]
pub struct MoneroSyncInteraction {
    pub request_start_time: i64,
    pub message: MoneroWalletMessage,
    pub message_type: MoneroWalletMessageType,
    pub peer_pks: Vec<PublicKey>,
    pub all_pks: Vec<PublicKey>,
    pub threshold: i64,
    pub peer_strings: Vec<String>,
    pub response: flume::Sender<RgResult<MoneroWalletResponse>>
}

impl MoneroSyncInteraction {
    pub fn address_descriptor_address_from_pk_threshold(&self) -> Address {
        Address::from_multisig_public_keys_and_threshold(
            &self.all_pks,
            self.threshold
        )
    }
    pub fn request(&self, peer_strings: Vec<String>) -> Request {
        let mut req = Request::default();
        req.monero_multisig_formation_request = Some(MoneroMultisigFormationRequest {
            public_keys: self.all_pks.clone(),
            threshold: Some(Weighting::from_int_basis(self.threshold, self.all_pks.len() as i64)),
            peer_strings
        });
        req
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct MoneroInstanceSecretData {
    pub wallet_group_key: Address,
    pub wallet_group_key_str: String,
    pub wallet_prefix_dir: String,
    pub working_dir: String,
    pub wallet_name: String,
    pub wallet_path: String,
    pub wallet_data: MoneroWalletDiskFiles,
    pub peer_strings: Vec<Vec<String>>,
    pub created_restore_height: i64,
    pub created_at: i64,
    pub last_wallet_height: i64,
    pub last_sync_time: i64,
    pub raw_monero_output_address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MoneroTransactionInfo {
    pub tx: Vec<MoneroCliHistoryTransaction>,
    pub last_updated: i64
}
#[derive(Clone, Debug)]
pub enum MoneroWalletResponse {
    PeerCreate(String),
    InstanceCreated(MoneroInstanceSecretData),
    // This also attempts to refresh the transactions if possible.
    Transactions(MoneroTransactionInfo)
}

#[async_trait]
pub trait MoneroWalletSender {
    async fn send(&self, message: MoneroWalletMessage) -> RgResult<()>;
}

pub(crate) const MULTISIG_WALLET_FILENAME: &str = "multisig_wallet";

impl<T> MoneroWalletSyncWriter<T> where T: PeerBroadcast
{

    pub async fn handle_first_ever_message(
        &mut self,
        message: MoneroSyncInteraction
    ) -> RgResult<()> {
        let valid_start_message =
            message.message_type == Formation &&
                (message.message == MoneroWalletMessage::InternalCreateMultisigAsProposer ||
            message.message == MoneroWalletMessage::HandleNextStageRequest);
        if !valid_start_message {
            message.response.send_rg_err("Invalid start message".to_error()).log_error().ok();
        }

        let t = self.new_thread();
        let (s, r) = flume::unbounded();
        let jh = tokio::spawn(async move {
            let mut t = t;
            let mut r = r;
            loop {
                if let Ok(m) = r.try_recv() {
                    t.handle_message(m).await.log_error().ok();
                }
            }
        });

        self.sync_handlers.insert(
            message.address_descriptor_address_from_pk_threshold(),
            IndividualWalletSyncHandlerThread {
                jh: Arc::new(jh),
                sender: s
            });
        Ok(())
    }
    pub async fn new_from_config(
        peer_broadcast: T,
        base_wallet_working_directory: PathBuf,
        daemon_address: String,
        words: WordsPass,
        secret_data: Vec<MoneroInstanceSecretData>
    ) -> Self {
        let mut writer = Self {
            sync_handlers: Default::default(),
            peer_broadcast,
            base_wallet_working_directory,
            daemon_address,
            words,
        };

        for data in secret_data {
            let address = data.wallet_group_key.clone();
            let t = writer.new_thread();
            let (s, r) = flume::unbounded();
            let jh = tokio::spawn(async move {
                let mut t = t;
                t.is_restore = true;
                let mut r = r;
                while t.live_wallet.is_err() {
                    t.live_wallet = LiveWallet::open_existing_wallet(
                        t.daemon_address.clone(),
                        data.clone()
                    ).await;
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                loop {
                    if let Ok(m) = r.try_recv() {
                        t.handle_message(m).await.log_error().ok();
                    }
                }
            });

            writer.sync_handlers.insert(
                address,
                IndividualWalletSyncHandlerThread {
                    jh: Arc::new(jh),
                    sender: s
                });
        }
        writer
    }

    pub fn new_thread(&self) -> WalletThread<T> {
        WalletThread {
            words: self.words.clone(),
            base_wallet_working_directory: self.base_wallet_working_directory.clone(),
            daemon_address: self.daemon_address.clone(),
            peer_broadcast: self.peer_broadcast.clone(),
            live_wallet: "CLI not started".to_error(),
            is_restore: false,
        }
    }
}

#[async_trait]
impl<B: 'static> TryRecvForEach<MoneroSyncInteraction> for MoneroWalletSyncWriter<B> where B: PeerBroadcast {
    async fn try_recv_for_each(&mut self, message: MoneroSyncInteraction) -> RgResult<()> {

        match self.sync_handlers.get(&message.address_descriptor_address_from_pk_threshold()) {
            None => {
                self.handle_first_ever_message(message).await?;
            }
            Some(h) => {
                h.sender.send_rg_err(message).log_error().ok();
            }
        }
        Ok(())
    }
}
