use redgold_keys::address_external::ToEthereumAddress;
use redgold_schema::conf::node_config::NodeConfig;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_schema::party::party_events::PartyEvents;
use redgold_schema::seeds::get_seeds_by_env;
use redgold_schema::structs::NetworkEnvironment;
use crate::core::relay::Relay;
use crate::node_config::{ApiNodeConfig, EnvDefaultNodeConfig};
use crate::party::party_stream::PartyEventBuilder;

#[tokio::test]
pub async fn debug() {
    if std::env::var("REDGOLD_DEBUG_DEVELOPER").is_err() {
        return;
    }
    let nc = NodeConfig::by_env_with_args(NetworkEnvironment::Dev).await;
    let a = nc.api_rg_client();
    let pd = a.party_data().await.unwrap();
    let pid = pd.into_iter().next().unwrap().1;
    let pev = pid.party_events.unwrap();
    let r = Relay::new(nc.clone()).await;
    let mut pev2 = PartyEvents::new(
        &NetworkEnvironment::Dev, &r, pev.party_addresses.clone()
    );

    println!("{}", pev.pending_external_staking_txs.len());
    println!("{}", pev.events.len());

    for ev in pev.events.iter() {
        println!("Processing event");
        let result = pev2.process_event(ev).await.unwrap();
    }
}