use std::fmt::{self, Display};
use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::{ParseError, Uuid};

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Default, PartialEq)]
#[sql_type = "SqlUuid"]
pub struct UserId(Uuid);
derive_newtype_sql!(user_id, SqlUuid, UserId, UserId);

impl UserId {
    pub fn new(id: Uuid) -> Self {
        UserId(id)
    }
    pub fn inner(&self) -> &Uuid {
        &self.0
    }
    pub fn generate() -> Self {
        UserId(Uuid::new_v4())
    }
}

impl FromStr for UserId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(UserId::new(id))
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{}", self.0.hyphenated()))
    }
}
