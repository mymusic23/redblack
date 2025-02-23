use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use redgold_keys::monero::wallet_cli::monero_wallet_cli::MoneroWalletCli;
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
use redgold_schema::{RgResult, SafeOption};
use redgold_schema::structs::{Address, AddressDescriptor, PartyId, PublicKey};
use redgold_schema::util::times::current_time_millis;
use tokio::time::Instant;


#[derive(Clone, Debug)]
pub enum MultisigFormationStage {
    // Creation of the multisig wallet
    Prepared,
    Made,
    Exchanged,
    // Transfer
    Export,
    Import,
    Sign
}

#[derive(Clone)]
pub struct ActiveMultisigOperation {
    pub last_peer_vecs: Vec<String>,
    pub cli: MoneroWalletCli,
    pub stage: MultisigFormationStage,
    pub started: Instant,
    pub operation: Arc<JoinHandle<RgResult<Option<String>>>>,
    pub last_self_peer_info: Option<String>
}

#[derive(Clone)]
pub struct MoneroWalletSyncWriter<B: PeerBroadcast + 'static> {
    // Must be kept active to keep wallet cli child process synchronized to monerod
    pub live_wallets: HashMap<Address, MoneroWalletCli>,
    // Mechanism to contact party peers
    pub peer_broadcast: B,
    // Since we don't yet have the wallet address, this address is from an address descriptor
    pub active_formations: HashMap<Address, ActiveMultisigOperation>,
    // This is the party multisig address
    pub active_transfers: HashMap<Address, ActiveMultisigOperation>
}

#[derive(Clone, Debug)]
pub enum MoneroWalletMessage {
    // This is the first step, the proposer starts the operation internally
    CreateMultsigAsProposer,
    // Next, the potential party members must initialize a fresh wallet 
    // and send the resulting string back to the proposer,
    // This message indicates we're a party member and receiving an external message
    // requesting us to create a new multisig wallet
    HandleMultisigFormationNextStageRequest,
    // Incoming message from proposer checking if we are prepared for next stage, respond with peer string 
    // if so
    HandleCheckStageComplete,
    // Internal only message
    // Proposer creates the multisig transfer request on attempting to fulfill an order
    CreateMultisigTransferAsProposer,
    // Initial trigger from proposer to start the next transfer stage
    // This message indicates we're a party member and receiving an external message
    HandleMultisigTransferNextStageRequest,
    // Check if the transfer stage is complete
    HandleMultisigTransferCheckStageComplete,

}

// This is the internal request type
#[derive(Clone, Debug)]
pub struct MoneroSyncInteraction {
    pub message: MoneroWalletMessage,
    pub all_pks: Vec<PublicKey>,
    pub peer_strings: Vec<String>,
    pub threshold: i64,
    pub response: flume::Sender<RgResult<MoneroWalletResponse>>,
    pub operation_initialization: bool
}

#[derive(Clone, Debug)]
pub enum MoneroWalletResponse {
    PeerCreate(String),
    InstanceCreate(PartySecretInstanceData)
}

#[async_trait]
pub trait MoneroWalletSender {
    async fn send(&self, message: MoneroWalletMessage) -> RgResult<()>;
}

// #[async_trait]
// impl<B: 'static> TryRecvForEach<MoneroSyncInteraction> for MoneroWalletSyncWriter<B> where B: PeerBroadcast {
//     async fn try_recv_for_each(&mut self, message: MoneroSyncInteraction) -> RgResult<()> {
//         let ct = current_time_millis();
//         if let Some(o) = &self.operation {
//             // Cleanup old operations.
//             if o.on_final_stage && o.join_handle.is_finished() {
//                 self.operation = None;
//             } else if (ct - o.started_at) > 1000 * 120 {
//                 o.join_handle.abort();
//                 self.operation = None;
//             }
//         }
//         if let Some(o) = &self.operation {
//             if message.operation_initialization {
//                 message.response.send_rg_err(
//                     "Operation initialization requested but operation already in progress".to_error()
//                 ).log_error().ok();
//                 return Ok(())
//             } else if message.wallet_id != o.id{
//                     message.response.send_rg_err(
//                         "Operation requested with different wallet id from the one in progress, busy".to_error()
//                     ).log_error().ok();
//                     return Ok(())
//             } else if !o.join_handle.is_finished() {
//                 message.response.send_rg_err(
//                     "Operation requested while join handle in progress, busy".to_error()
//                 ).log_error().ok();
//                 return Ok(())
//             }
//         }
//         match message.message {
//             MoneroWalletMessage::CreateMultsigAsProposer => {
//                 if self.operation.is_some() {
//                     message.response.send_rg_err(
//                         "Operation requested while join handle in progress, busy".to_error()
//                     ).log_error().ok();
//                     return Ok(())
//                 }
//                 self.wallet_interface.lock().await.reset();
//                 let all_pks = message.all_pks.clone();
//                 let threshold = message.threshold;
//                 let peer_broadcast = self.peer_broadcast.clone();
//                 let sender = message.response.clone();
//                 let iface = self.wallet_interface.clone();
//                 let jh = tokio::spawn(async move {
//                     let result = iface.lock().await.multisig_create_loop(
//                         &all_pks,
//                         threshold,
//                         &peer_broadcast
//                     ).await.map(|x| {
//                         MoneroWalletResponse::InstanceCreate(x)
//                     });
//                     sender.send_rg_err(result).log_error().ok();
//                 });
//                 self.operation = Some(ActiveOperation{
//                     id: message.wallet_id.clone(),
//                     started_at: ct,
//                     join_handle: Arc::new(jh),
//                     on_final_stage: true,
//                 });

//             }
//             MoneroWalletMessage::MultisigCreateNext => {
//                 if message.operation_initialization {
//                     if self.operation.is_some() {
//                         message.response.send_rg_err(
//                             "Operation initialization requested but operation already in progress".to_error()
//                         ).log_error().ok();
//                         return Ok(())
//                     }
//                     self.wallet_interface.lock().await.reset();
//                 }
//                 let peer_strings = Some(message.peer_strings);
//                 let thresh = Some(message.threshold);
//                 let wallet_id = message.wallet_id.clone();
//                 let sender = message.response.clone();
//                 let final_state = self.wallet_interface.lock().await.state.is_before_final_state();
//                 let iface = self.wallet_interface.clone();
//                 let jh = tokio::spawn(async move {
//                     let result = iface.lock().await
//                         .multisig_create_next(peer_strings, thresh, &wallet_id, true, 2)
//                         .await;
//                     let response = result.and_then(|r| r.multisig_info_string().ok_msg("No multisig info string"))
//                         .map(|x| MoneroWalletResponse::PeerCreate(x));
//                     sender.send_rg_err(response).log_error().ok();
//                 });

//                 if message.operation_initialization {
//                     self.operation = Some(ActiveOperation {
//                         id: message.wallet_id.clone(),
//                         started_at: ct,
//                         join_handle: Arc::new(jh),
//                         on_final_stage: final_state,
//                     })
//                 } else if let Some(o) = &mut self.operation {
//                     o.join_handle = Arc::new(jh);
//                     o.on_final_stage = final_state;
//                 } else {
//                     message.response.send_rg_err(
//                         "No operation initialization requested but no operation in progress".to_error()
//                     ).log_error().ok();
//                 }
//             }
//         }
//         Ok(())
//     }
// }
