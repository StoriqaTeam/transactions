use models::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename = "camelCase")]
pub struct Metrics {
    pub accounts_count: HashMap<UserId, u64>,
    pub accounts_count_total: u64,
    pub total_balance: HashMap<Currency, f64>,
    pub total_blockchain_balance: HashMap<Currency, f64>,
    pub fees_balances: HashMap<Currency, f64>,
    pub liquidity_balances: HashMap<Currency, f64>,
    pub number_of_negative_balances: u64,
    pub limits: HashMap<Currency, f64>,
}
