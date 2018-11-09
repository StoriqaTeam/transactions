use models::*;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinFeeResponse {
    pub fastest_fee: f32,
    pub half_hour_fee: f32,
    pub hour_fee: f32,
}

impl BitcoinFeeResponse {
    pub fn to_fees(self, btc_transaction_size: i32) -> Vec<Fee> {
        let mut result = vec![];
        let one_dimension = (self.fastest_fee - self.hour_fee) / 6f32;
        for i in 0..6 {
            let fee = self.hour_fee + (i as f32) * one_dimension;
            let value = Amount::new((fee * (btc_transaction_size as f32)) as u128);
            let estimated_time = 3600 - i * 600; // one hour is max
            let r = Fee { value, estimated_time };
            result.push(r);
        }
        result
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EthFeeResponse {
    pub safe_low: f32,
    pub standard: f32,
    pub fast: f32,
    pub fastest: f32,
}

const GWEI_DECIMALS: u128 = 1_000_000u128;

impl EthFeeResponse {
    pub fn to_fees(self, gas_limit: i32) -> Vec<Fee> {
        let mut result = vec![];
        let one_dimension = (self.fastest - self.safe_low) / 10f32;
        for i in 0..10 {
            let fee = self.safe_low + (i as f32) * one_dimension;
            let value = Amount::new(((fee * (gas_limit as f32)) as u128) * GWEI_DECIMALS);
            let estimated_time = 310 - i * 10;
            let r = Fee { value, estimated_time };
            result.push(r);
        }
        result
    }
}
