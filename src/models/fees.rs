use models::*;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fees {
    pub currency: Currency,
    pub fees: Vec<Fee>,
}

impl Default for Fees {
    fn default() -> Self {
        Self {
            currency: Currency::Eth,
            fees: vec![],
        }
    }
}

impl Fees {
    pub fn new(currency: Currency, fees: Vec<Fee>) -> Self {
        Self { currency, fees }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Fee {
    pub value: Amount,
    pub estimated_time: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFees {
    pub from_currency: Currency,
    pub to_currency: Currency,
    pub account_address: BlockchainAddress,
}

impl Default for GetFees {
    fn default() -> Self {
        Self {
            from_currency: Currency::Eth,
            to_currency: Currency::Btc,
            account_address: BlockchainAddress::default(),
        }
    }
}
