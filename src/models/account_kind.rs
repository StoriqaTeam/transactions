use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Cr,
    Dr,
}

impl FromSql<VarChar, Pg> for AccountKind {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"cr") => Ok(AccountKind::Cr),
            Some(b"dr") => Ok(AccountKind::Dr),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            ).to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for AccountKind {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            AccountKind::Cr => out.write_all(b"cr")?,
            AccountKind::Dr => out.write_all(b"dr")?,
        };
        Ok(IsNull::No)
    }
}
