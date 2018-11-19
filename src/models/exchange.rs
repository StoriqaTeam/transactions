use super::{Amount, Currency};

use chrono::NaiveDateTime;
use std::fmt::{self, Debug, Display};

use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

#[derive(Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, Hash)]
#[sql_type = "SqlUuid"]
pub struct ExchangeId(Uuid);
derive_newtype_sql!(account_id, SqlUuid, ExchangeId, ExchangeId);

impl Debug for ExchangeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Display::fmt(&self.0, f)
    }
}

impl Default for ExchangeId {
    fn default() -> Self {
        ExchangeId(Uuid::new_v4())
    }
}

impl ExchangeId {
    pub fn new(id: Uuid) -> Self {
        ExchangeId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn generate() -> Self {
        ExchangeId(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeInput {
    pub id: ExchangeId,
    pub from: Currency,
    pub to: Currency,
    pub rate: f64,
    pub actual_amount: Amount,
    pub amount_currency: Currency,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    pub from: Currency,
    pub to: Currency,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateInput {
    pub id: ExchangeId,
    pub from: Currency,
    pub to: Currency,
    pub amount: Amount,
    pub amount_currency: Currency,
}

impl RateInput {
    pub fn new(from: Currency, to: Currency, amount: Amount, amount_currency: Currency) -> Self {
        Self {
            id: ExchangeId::generate(),
            from,
            to,
            amount,
            amount_currency,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    pub id: ExchangeId,
    pub from: Currency,
    pub to: Currency,
    pub amount: Amount,
    pub amount_currency: Currency,
    pub rate: f64,
    pub expiration: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
