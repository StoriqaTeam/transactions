use std::fmt::{self, Display};
use std::io::Write;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum DailyLimitType {
    DefaultLimit,
    Unlimited,
}

impl Default for DailyLimitType {
    fn default() -> Self {
        DailyLimitType::DefaultLimit
    }
}

impl FromSql<VarChar, Pg> for DailyLimitType {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"default") => Ok(DailyLimitType::DefaultLimit),
            Some(b"unlimited") => Ok(DailyLimitType::Unlimited),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            )
            .to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for DailyLimitType {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            DailyLimitType::DefaultLimit => out.write_all(b"default")?,
            DailyLimitType::Unlimited => out.write_all(b"unlimited")?,
        };
        Ok(IsNull::No)
    }
}

impl Display for DailyLimitType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DailyLimitType::DefaultLimit => f.write_str("default"),
            DailyLimitType::Unlimited => f.write_str("unlimited"),
        }
    }
}
