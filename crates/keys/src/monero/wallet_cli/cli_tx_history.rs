use std::env::home_dir;
use crate::monero::wallet_cli::monero_wallet_cli::MoneroWalletCli;

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use chrono::{DateTime, NaiveDateTime, Utc};
use uuid::Uuid;
use redgold_common_no_wasm::readers_writers::FileUtils;
use redgold_schema::{ErrorInfoContext, RgResult, SafeOption};
use redgold_schema::errors::into_error::ToErrorInfo;
use redgold_schema::helpers::easy_json::EasyJson;
use redgold_schema::observability::errors::EnhanceErrorInfo;
use redgold_schema::party::address_event::AddressEvent::External;
use redgold_schema::structs::{Address, CurrencyAmount, SupportedCurrency};
use redgold_schema::tx::external_tx::ExternalTimedTransaction;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MoneroCliHistoryTransaction {
    block: u64,
    direction: String,
    unlocked: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "amount")]
    primary_amount: f64,
    #[serde(rename = "running balance")]
    running_balance: f64,
    hash: String,
    #[serde(rename = "payment ID")]
    payment_id: String,
    fee: f64,
    destination: String,
    #[serde(rename = "amount")]  // Second amount field
    secondary_amount: Option<f64>,
    #[serde(rename = "index")]
    amount_index: Option<String>,
    note: String,
    #[serde(rename = "tx key")]
    tx_key: Option<String>,
}

impl MoneroCliHistoryTransaction {

    pub fn time_ms(&self) -> RgResult<i64> {
        let timestamp_ms = self.timestamp.as_ref()
            .and_then(|ts| NaiveDateTime::parse_from_str(ts.trim(), "%Y-%m-%d %H:%M:%S").ok())
            .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
            .map(|dt| dt.timestamp_millis())
            .ok_msg("timestamp parse")?;
        Ok(timestamp_ms)
    }

    // 2025-02-21 03:21:30 timestamp format

    pub fn is_unlocked(&self) -> bool {
        self.unlocked.as_ref().map(|u| u == "unlocked").unwrap_or(false)
    }

    pub fn destination(&self) -> Option<Address> {
        let dest = self.destination.trim();
        if dest.is_empty() || dest.eq("-") {
            None
        } else {
            Some(Address::from_monero_external(dest))
        }
    }

    pub fn to_external_tx(&self) -> RgResult<ExternalTimedTransaction> {
        self.to_external_tx_inner().with_detail("raw", self.json_or())
    }
    pub fn to_external_tx_inner(&self) -> RgResult<ExternalTimedTransaction> {

        let amount = CurrencyAmount::from_fractional_cur(self.primary_amount, SupportedCurrency::Monero)?;
        let fee = CurrencyAmount::from_fractional_cur(self.fee, SupportedCurrency::Monero)?;

        let dest = self.destination();
        let mut ett = ExternalTimedTransaction {
            tx_id: self.hash.clone(),
            timestamp: Some(self.time_ms()? as u64),
            other_address: "".to_string(),
            other_output_addresses: vec![],
            amount: (amount.to_fractional() * 1e8) as u64,
            bigint_amount: None,
            incoming: self.direction == "in",
            currency: SupportedCurrency::Monero,
            block_number: Some(self.block),
            price_usd: None,
            fee: Some(fee),
            self_address: None,
            currency_id: Some(SupportedCurrency::Monero.to_currency_id()),
            currency_amount: Some(amount.clone()),
            from: Default::default(),
            to: vec![],
            other: None,
            queried_address: None,
        };
        if ett.incoming {
            ett.from = dest.ok_msg("from address")?;
            ett.self_address = Some(ett.from.render_string()?);
        }
        Ok(ett)
    }
}

pub fn parse_output<P: AsRef<Path>>(path: P) -> RgResult<Vec<MoneroCliHistoryTransaction>> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .has_headers(true)
        // This is important - tells CSV reader to ignore duplicate fields
        .double_quote(true)
        .from_path(path)
        .error_info("path")?;

    let mut res = vec![];
    for result in rdr.records() {
        let record = result.error_info("record read")?;
        let transaction = MoneroCliHistoryTransaction {
            block: record.get(0).unwrap_or("0").trim().parse().unwrap_or(0),
            direction: record.get(1).unwrap_or("").trim().to_string(),
            unlocked: Some(record.get(2).unwrap_or("").trim().to_string()),
            timestamp: Some(record.get(3).unwrap_or("").trim().to_string()),
            primary_amount: record.get(4).unwrap_or("0").trim().parse().unwrap_or(0.0),
            running_balance: record.get(5).unwrap_or("0").trim().parse().unwrap_or(0.0),
            hash: record.get(6).unwrap_or("").trim().to_string(),
            payment_id: record.get(7).unwrap_or("").trim().to_string(),
            fee: record.get(8).unwrap_or("0").trim().parse().unwrap_or(0.0),
            destination: record.get(9).unwrap_or("").trim().to_string(),
            secondary_amount: record.get(10).unwrap_or("").trim().parse().ok(),
            amount_index: Some(record.get(11).unwrap_or("").trim().to_string()),
            note: record.get(12).unwrap_or("").trim().to_string(),
            tx_key: Some(record.get(13).unwrap_or("").trim().to_string()),
        };
        res.push(transaction);
    }
    Ok(res)
}


#[tokio::test]
async fn debug_csv_parse() {

    let sample_text = r#"   block,direction,unlocked,                timestamp,              amount,     running balance,                                                            hash,      payment ID,           fee,                                                                                               destination,              amount,index,note,tx key
 3338546,       in,unlocked,      2025-02-02 04:08:03,      0.010000000000,      0.010000000000,89a37a3ff2dee2e3faf8a711b1cb534be8be1769ce6773e945e084b4b10c2657,0000000000000000,0.000000000000,           46AYBkASoYPENtzG1A6fpVQooVkxoXJokJuz1MZzMzVK4XfhULjDEVB8UGpfHhFpgXBkBbUeRdKEZJArLJqR3ZF3UNyJYFr,      0.010000000000,"0",,
 3349190,      out,       -,      2025-02-16 23:16:06,      0.002000000000,      0.007969340000,464b15a86fa43675d2d9f5b36a4ca212f2a6385ff1800bb5312f29d844b80f1a,0000000000000000,0.000030660000,                                                                                                         -,                    ,"0",,
 3349219,      out,       -,      2025-02-17 00:28:56,      0.002000000000,      0.005938640000,5b14e44617f508888fc78b1f55012c89363dee0ea1b12cb91367c0bfffffc188,0000000000000000,0.000030700000,                                                                                                         -,                    ,"0",,
 3351588,      out,       -,      2025-02-20 06:16:21,      0.001000000000,      0.004907960000,d3efac66e2dc695a9c9d1544afad962974654842d0a121097d077b79f82ad376,0000000000000000,0.000030680000,                                                                                                         -,                    ,"0",,
 3352221,       in,unlocked,      2025-02-21 03:21:30,      0.200000000000,      0.204907960000,625157275200a271c1461050039bd46b29117175a8ef2fe91c398fe8803f42ea,0000000000000000,0.000000000000,           46AYBkASoYPENtzG1A6fpVQooVkxoXJokJuz1MZzMzVK4XfhULjDEVB8UGpfHhFpgXBkBbUeRdKEZJArLJqR3ZF3UNyJYFr,      0.200000000000,"0",,
 3352263,      out,       -,      2025-02-21 05:05:55,      0.010000000000,      0.194863520000,003f349023d583ff2d84c88d37fbbb799c2c729b60b763a21829bb665bbdfc70,0000000000000000,0.000044440000,                                                                                                         -,                    ,"0",,
"#;

    let path = "test_file.csv";
    path.write_string(sample_text).await.unwrap();
    // let home = home_dir().unwrap();
    // let path = home.join("test.csv");
    // let path = path.to_str().unwrap().to_string();
    let res = parse_output(path);
    for r in res.unwrap() {
        println!("{:?}", r);
        println!("{}", r.to_external_tx().unwrap().json_or());
    }

}

impl MoneroWalletCli {

    pub async fn export_transfers(&self) -> RgResult<Vec<MoneroCliHistoryTransaction>> {
        let output_file = format!("{}.csv", Uuid::new_v4().to_string());
        // export_transfers all output=test.csv
        // CSV exported to
        self.write(format!("export_transfers all output={}", output_file.clone())).await?;
        self.try_read_expect("CSV exported to").await?;
        let output = parse_output(&output_file)?;
        output_file.delete_file().await?;
        Ok(output)
    }
}