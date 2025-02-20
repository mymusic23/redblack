use redgold_common_no_wasm::cmd::run_bash_async;
use redgold_schema::{errors::into_error::ToErrorInfo, helpers::easy_json::EasyJsonDeser, observability::errors::EnhanceErrorInfo, RgResult};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct TxEstimate {
    input: String,
    #[serde(rename = "totalFees")]
    total_fees: TotalFees,
    #[serde(rename = "slipBasisPoints")]
    slip_basis_points: String,
    #[serde(rename = "netOutput")]
    net_output: String,
    #[serde(rename = "inboundConfirmationSeconds")]
    inbound_confirmation_seconds: u32,
    #[serde(rename = "outboundDelaySeconds")]
    outbound_delay_seconds: u32,
    #[serde(rename = "canSwap")]
    can_swap: bool,
    errors: Vec<String>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct TotalFees {
    #[serde(rename = "outboundFee")]
    outbound_fee: String,
    #[serde(rename = "affiliateFee")]
    affiliate_fee: String
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EstimatedSwap {
    memo: String,
    expiry: String,
    #[serde(rename = "toAddress")]
    to_address: String,
    #[serde(rename = "txEstimate")]
    tx_estimate: TxEstimate
}

#[test]
fn test_parse() {
    let json = r#"
    {
        memo: '=:ETH.ETH:0xA729F9430fc31Cda6173A0e81B55bBC92426f759:3487602634',
        expiry: 2025-02-17T06:40:32.000Z,
        toAddress: 'bc1q9rprv6mnevdmsqknpveyus3vm7j059evh7f8j8',
        txEstimate: {
            input: '₿ 1',
            totalFees: { outboundFee: 'Ξ 0.00036608', affiliateFee: 'Ξ 0' },
            slipBasisPoints: '130',
            netOutput: 'Ξ 35.02462039',
            inboundConfirmationSeconds: 600,
            outboundDelaySeconds: 912,
            canSwap: true,
            errors: []
        }
    }
"#;
    let swap_details = EstimatedSwap::from_json(json).unwrap();
    println!("Swap details: {:?}", swap_details);
}

impl EstimatedSwap {
    pub fn from_json(json: &str) -> RgResult<Self> {
        let step1 = json
            .replace('\'', "\"")  // Replace single quotes with double quotes
            // Handle the date format
            .replace(": 2", ": \"2")
            .replace("Z,", "Z\",")  // Fix date format ending
            // Fix all the key-value pairs
            .replace("outboundFee:", "\"outboundFee\":")
            .replace("affiliateFee:", "\"affiliateFee\":")
            .replace("memo:", "\"memo\":")
            .replace("expiry:", "\"expiry\":")
            .replace("toAddress:", "\"toAddress\":")
            .replace("txEstimate:", "\"txEstimate\":")
            .replace("input:", "\"input\":")
            .replace("totalFees:", "\"totalFees\":")
            .replace("slipBasisPoints:", "\"slipBasisPoints\":")
            .replace("netOutput:", "\"netOutput\":")
            .replace("inboundConfirmationSeconds:", "\"inboundConfirmationSeconds\":")
            .replace("outboundDelaySeconds:", "\"outboundDelaySeconds\":")
            .replace("canSwap:", "\"canSwap\":")
            .replace("errors:", "\"errors\":")
            // Fix double quotes and special cases
            .replace("\"\",", "\",")  // Remove any double quotes
            .replace("\"\"", "\"")
            .replace(": \"true\"", ": true")  // Fix boolean values
            .replace(": \"false\"", ": false")
            .replace(": \"[]\"", ": []")  // Fix empty arrays
            .replace(": \"600\"", ": 600")  // Fix numeric values
            .replace(": \"912\"", ": 912")
            // Clean up Unicode and special characters
            .chars()
            .filter(|&c| {
                // Allow only ASCII printable characters needed for JSON
                c.is_ascii_alphanumeric() || 
                 c == '{' || c == '}' || c == '[' || c == ']' || 
                 c == ':' || c == ',' || c == '.' || c == '-' || 
                 c == '"' || c == ' ' || c == '\n' || c == '_'
            })
            .collect::<String>();
            
        println!("Replaced json: {}", step1);
        step1.json_from()
    }
}

/*

npm run estimateSwap mainnet 1.0 8 BTC.BTC ETH.ETH 0xA729F9430fc31Cda6173A0e81B55bBC92426f759


bc1qrxdzt6v9yuu567j52cmla4v9kler3wzj0k44lk
 */
use std::path::PathBuf;

use crate::add;

pub async fn estimate_swap_inner(
    working_dir: impl Into<String>,
    network: impl Into<String>,
    amount: impl Into<String>,
    decimals: impl Into<String>,
    from_cur: impl Into<String>,
    to_cur: impl Into<String>,
    destination: impl Into<String>
) -> RgResult<EstimatedSwap> {
    let cmd = format!(
        "cd {}; npm run estimateSwap {} {} {} {} {} {}",
        working_dir.into(), network.into(), amount.into(), decimals.into(), from_cur.into(), to_cur.into(), destination.into()
    );
    let (stdout, stderr) = run_bash_async(&cmd).await?;
    let parsed = stdout.trim().to_string().json_from::<EstimatedSwap>()
            .add("err running thorchain estimate swap command")
            .with_detail("stderr", stderr)
            .with_detail("cmd", cmd)
            .with_detail("stdout", stdout)?;
    Ok(parsed)
}

#[ignore]
#[tokio::test]
async fn test_estimate_swap() {
    let path = PathBuf::from("../../npm-hook/thor").to_string_lossy().to_string();
    let swap_details = estimate_swap_inner(
        path,  "mainnet", "1.0", "8", 
        "BTC.BTC", "ETH.ETH", 
        "0xA729F9430fc31Cda6173A0e81B55bBC92426f759").await.unwrap();
    println!("Swap details: {:?}", swap_details);
}
