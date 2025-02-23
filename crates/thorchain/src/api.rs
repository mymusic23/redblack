/*
curl -X 'GET' \
  'https://thornode.ninerealms.com/thorchain/quote/swap?from_asset=BTC.BTC&to_asset=ETH.ETH&amount=1000000&destination=0xA729F9430fc31Cda6173A0e81B55bBC92426f759' \
  -H 'accept: application/json'
   */

use redgold_schema::{helpers::easy_json::EasyJsonDeser, ErrorInfoContext, RgResult};
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};

pub async fn thornode_estimate_swap_api_request(
    from_asset: &str,
    to_asset: &str,
    amount: u64,
    destination: &str,
) -> RgResult<SwapQuoteResponse> {
    let c = ClientBuilder::new().build()
    .error_info("Failed to create client")?;
    let url = format!("https://thornode.ninerealms.com/thorchain/quote/swap?from_asset={}&to_asset={}&amount={}&destination={}", from_asset, to_asset, amount, destination);
    let resp = c.get(url).send()
    .await.error_info("Failed to send request")?;
    let text = resp.text().await.error_info("Failed to get text")?;
    let quote: SwapQuoteResponse = text.json_from()?;
    Ok(quote)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SwapFees {
    pub asset: String,
    pub affiliate: String,
    pub outbound: String,
    pub liquidity: String,
    pub total: String,
    pub slippage_bps: u64,
    pub total_bps: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SwapQuoteResponse {
    pub inbound_address: String,
    pub inbound_confirmation_blocks: u64,
    pub inbound_confirmation_seconds: u64,
    pub outbound_delay_blocks: u64,
    pub outbound_delay_seconds: u64,
    pub fees: SwapFees,
    pub expiry: u64,
    pub warning: String,
    pub notes: String,
    pub dust_threshold: String,
    pub recommended_min_amount_in: String,
    pub recommended_gas_rate: String,
    pub gas_rate_units: String,
    pub memo: String,
    pub expected_amount_out: String,
    pub max_streaming_quantity: u64,
    pub streaming_swap_blocks: u64,
    pub total_swap_seconds: u64,
}

#[tokio::test]
async fn test_thornode_estimate_swap_api_request() {
    let resp = 
    thornode_estimate_swap_api_request("BTC.BTC", "ETH.ETH", 1000000, "0xA729F9430fc31Cda6173A0e81B55bBC92426f759").await;
    println!("{:?}", resp.unwrap());
}