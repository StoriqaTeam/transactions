use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{self, Uuid};

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, Hash)]
#[sql_type = "SqlUuid"]
pub struct AccountId(Uuid);
derive_newtype_sql!(account_id, SqlUuid, AccountId, AccountId);

impl AccountId {
    pub fn new(id: Uuid) -> Self {
        AccountId(id)
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn generate() -> Self {
        AccountId(Uuid::new_v4())
    }
}

impl FromStr for AccountId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(AccountId::new(id))
    }
}

impl Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}
