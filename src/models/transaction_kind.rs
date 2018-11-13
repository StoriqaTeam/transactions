use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use std::io::Write;

#[derive(Debug, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
pub enum TransactionGroupKind {
    Deposit,
    Internal,
    InternalMulti,
    Withdrawal,
    WithdrawalMulti,
    Approval,
}

impl FromSql<VarChar, Pg> for TransactionGroupKind {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"deposit") => Ok(TransactionGroupKind::Deposit),
            Some(b"internal") => Ok(TransactionGroupKind::Internal),
            Some(b"internal_multi") => Ok(TransactionGroupKind::InternalMulti),
            Some(b"withdrawal") => Ok(TransactionGroupKind::Withdrawal),
            Some(b"withdrawal_multi") => Ok(TransactionGroupKind::WithdrawalMulti),
            Some(b"approval") => Ok(TransactionGroupKind::Approval),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            ).to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for TransactionGroupKind {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            TransactionGroupKind::Deposit => out.write_all(b"deposit")?,
            TransactionGroupKind::Internal => out.write_all(b"internal")?,
            TransactionGroupKind::InternalMulti => out.write_all(b"internal_multi")?,
            TransactionGroupKind::Withdrawal => out.write_all(b"withdrawal")?,
            TransactionGroupKind::WithdrawalMulti => out.write_all(b"withdrawal_multi")?,
            TransactionGroupKind::Approval => out.write_all(b"approval")?,
        };
        Ok(IsNull::No)
    }
}

#[derive(Debug, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
pub enum TransactionKind {
    Fee,
    BlockchainFee,
    MultiFrom,
    MultiTo,
    Internal,
    Deposit,
    Withdrawal,
}

impl FromSql<VarChar, Pg> for TransactionKind {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"fee") => Ok(TransactionKind::Fee),
            Some(b"blockchain_fee") => Ok(TransactionKind::BlockchainFee),
            Some(b"multi_from") => Ok(TransactionKind::MultiFrom),
            Some(b"multi_to") => Ok(TransactionKind::MultiTo),
            Some(b"internal") => Ok(TransactionKind::Internal),
            Some(b"withdrawal") => Ok(TransactionKind::Withdrawal),
            Some(b"deposit") => Ok(TransactionKind::Deposit),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            ).to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for TransactionKind {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            TransactionKind::Fee => out.write_all(b"fee")?,
            TransactionKind::BlockchainFee => out.write_all(b"blockchain_fee")?,
            TransactionKind::MultiFrom => out.write_all(b"multi_from")?,
            TransactionKind::MultiTo => out.write_all(b"multi_to")?,
            TransactionKind::Internal => out.write_all(b"internal")?,
            TransactionKind::Deposit => out.write_all(b"deposit")?,
            TransactionKind::Withdrawal => out.write_all(b"withdrawal")?,
        };
        Ok(IsNull::No)
    }
}
