use std::str::FromStr;

use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Default)]
#[sql_type = "SqlUuid"]
pub struct UserId(Uuid);
derive_newtype_sql!(user_id, SqlUuid, UserId, UserId);

impl UserId {
    pub fn new(id: Uuid) -> Self {
        UserId(id)
    }
}

impl UserId {
    pub fn inner(&self) -> &Uuid {
        &self.0
    }
}

impl FromStr for UserId {
    type Err = uuid::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s)?;
        Ok(UserId::new(id))
    }
}
