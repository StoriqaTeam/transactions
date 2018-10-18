use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Done,
}

impl FromSql<VarChar, Pg> for TransactionStatus {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"pending") => Ok(TransactionStatus::Pending),
            Some(b"done") => Ok(TransactionStatus::Done),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            ).to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for TransactionStatus {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            TransactionStatus::Pending => out.write_all(b"pending")?,
            TransactionStatus::Done => out.write_all(b"done")?,
        };
        Ok(IsNull::No)
    }
}
