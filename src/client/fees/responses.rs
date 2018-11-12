use std::fmt;
use std::num::ParseFloatError;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use models::*;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinFeeResponse {
    pub fastest_fee: f64,
    pub half_hour_fee: f64,
    pub hour_fee: f64,
}

impl BitcoinFeeResponse {
    pub fn to_fees(self, btc_transaction_size: i32) -> Vec<Fee> {
        let mut result = vec![];
        let one_dimension = (self.fastest_fee - self.hour_fee) / 6f64;
        for i in 0..6 {
            let fee = self.hour_fee + (i as f64) * one_dimension;
            let value = Amount::new((fee * (btc_transaction_size as f64)) as u128);
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
    #[serde(deserialize_with = "string_to_f64")]
    pub safe_low: f64,
    #[serde(deserialize_with = "string_to_f64")]
    pub standard: f64,
    #[serde(deserialize_with = "string_to_f64")]
    pub fast: f64,
    #[serde(deserialize_with = "string_to_f64")]
    pub fastest: f64,
}

const GWEI_DECIMALS: u128 = 1_000_000u128;

impl EthFeeResponse {
    pub fn to_fees(self, gas_limit: i32) -> Vec<Fee> {
        let mut result = vec![];
        let one_dimension = (self.fastest - self.safe_low) / 10f64;
        for i in 0..10 {
            let fee = self.safe_low + (i as f64) * one_dimension;
            let value = Amount::new(((fee * (gas_limit as f64)) as u128) * GWEI_DECIMALS);
            let estimated_time = 280 - i * 30;
            let r = Fee { value, estimated_time };
            result.push(r);
        }
        result
    }
}

fn string_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringTof64;

    impl<'de> Visitor<'de> for StringTof64 {
        type Value = f64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<f64, E>
        where
            E: de::Error,
        {
            value
                .parse()
                .map_err(|e: ParseFloatError| de::Error::invalid_type(de::Unexpected::Other(&e.to_string()), &self))
        }

        fn visit_map<M>(self, visitor: M) -> Result<f64, M::Error>
        where
            M: MapAccess<'de>,
        {
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(visitor))
        }
    }

    deserializer.deserialize_any(StringTof64)
}
