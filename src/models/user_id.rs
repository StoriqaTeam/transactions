use std::io::Write;
use std::str::FromStr;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Uuid as SqlUuid;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy)]
#[sql_type = "SqlUuid"]
pub struct UserId(Uuid);

impl FromSql<SqlUuid, Pg> for UserId {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        FromSql::<SqlUuid, Pg>::from_sql(data).map(UserId)
    }
}

impl ToSql<SqlUuid, Pg> for UserId {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        ToSql::<SqlUuid, Pg>::to_sql(&self.0, out)
    }
}

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
