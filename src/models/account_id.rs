use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{self, Uuid};

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Default, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct AccountId(Uuid);
derive_newtype_sql!(user_id, SqlUuid, AccountId, AccountId);

impl AccountId {
    pub fn new(id: Uuid) -> Self {
        AccountId(id)
    }
}

impl AccountId {
    pub fn inner(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for AccountId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(AccountId::new(id))
    }
}
