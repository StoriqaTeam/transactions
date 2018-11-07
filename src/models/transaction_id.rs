use std::fmt::{self, Debug, Display};
use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{self, Uuid};

#[derive(Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, Hash)]
#[sql_type = "SqlUuid"]
pub struct TransactionId(Uuid);
derive_newtype_sql!(transaction_id, SqlUuid, TransactionId, TransactionId);

impl Debug for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Display::fmt(&self.0, f)
    }
}

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

    pub fn next(&self) -> Self {
        let mut bytes = self.0.as_bytes().to_vec();
        let last = bytes.len() - 1;
        bytes[last] = bytes[last].wrapping_add(1);
        let uuid = Uuid::from_bytes(&bytes).unwrap();
        TransactionId(uuid)
    }

    pub fn prev(&self) -> Self {
        let mut bytes = self.0.as_bytes().to_vec();
        let last = bytes.len() - 1;
        bytes[last] = bytes[last].wrapping_sub(1);
        let uuid = Uuid::from_bytes(&bytes).unwrap();
        TransactionId(uuid)
    }

    pub fn last_byte(&self) -> u8 {
        let mut bytes = self.0.as_bytes().to_vec();
        let last = bytes.len() - 1;
        bytes[last]
    }
}

impl FromStr for TransactionId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(TransactionId::new(id))
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}
