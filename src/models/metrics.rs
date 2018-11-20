use models::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename = "camelCase")]
pub struct Metrics {
    pub accounts_count: HashMap<String, u64>,
    pub accounts_count_total: u64,
    pub total_payments_system_balances: HashMap<Currency, f64>,
    pub total_blockchain_balances: HashMap<Currency, f64>,
    pub fees_balances: HashMap<Currency, f64>,
    pub liquidity_balances: HashMap<Currency, f64>,
    pub limits: HashMap<Currency, f64>,
    pub diverging_blockchain_balances: Vec<DivergingBalance>,
    pub diverging_blockchain_balances_total: HashMap<Currency, f64>,
    pub negative_balances: Vec<NegativeBalance>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename = "camelCase")]
pub struct DivergingBalance {
    pub address: BlockchainAddress,
    pub currency: Currency,
    pub payments_system_value: f64,
    pub blockchain_value: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename = "camelCase")]
pub struct NegativeBalance {
    pub address: BlockchainAddress,
    pub currency: Currency,
    pub value: Amount,
}
