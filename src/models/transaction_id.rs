use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{self, Uuid};

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct TransactionId(Uuid);
derive_newtype_sql!(transaction_id, SqlUuid, TransactionId, TransactionId);

impl TransactionId {
    pub fn new(id: Uuid) -> Self {
        TransactionId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn generate() -> Self {
        TransactionId(Uuid::new_v4())
    }
}

impl FromStr for TransactionId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(TransactionId::new(id))
    }
}
