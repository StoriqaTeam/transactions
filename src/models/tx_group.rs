use std::fmt::{self, Debug, Display};
use std::io::Write;

use chrono::NaiveDateTime;
use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Uuid as SqlUuid;
use diesel::sql_types::VarChar;
use uuid::Uuid;
use validator::Validate;

use super::UserId;
use super::{BlockchainTransactionId, TransactionId, TransactionStatus, UserId};
use schema::tx_groups;

#[derive(Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, PartialEq, Eq, Hash)]
#[sql_type = "SqlUuid"]
pub struct TxGroupId(Uuid);
derive_newtype_sql!(transaction_id, SqlUuid, TxGroupId, TxGroupId);

impl From<TransactionId> for TxGroupId {
    fn from(tid: TransactionId) -> Self {
        TxGroupId(*tid.inner())
    }
}

impl Debug for TxGroupId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "lowercase")]
pub enum TxGroupKind {
    Deposit,
    Internal,
    InternalMulti,
    Withdrawal,
    WithdrawalMulti,
}

impl FromSql<VarChar, Pg> for TxGroupKind {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"internal") => Ok(TxGroupKind::Internal),
            Some(b"internal_multi") => Ok(TxGroupKind::InternalMulti),
            Some(b"withdrawal") => Ok(TxGroupKind::Withdrawal),
            Some(b"withdrawal_multi") => Ok(TxGroupKind::WithdrawalMulti),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            ).to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for TxGroupKind {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            TxGroupKind::Internal => out.write_all(b"internal")?,
            TxGroupKind::InternalMulti => out.write_all(b"internal_multi")?,
            TxGroupKind::Withdrawal => out.write_all(b"withdrawal")?,
            TxGroupKind::WithdrawalMulti => out.write_all(b"withdrawal_multi")?,
        };
        Ok(IsNull::No)
    }
}

#[derive(Debug, Queryable, Clone)]
pub struct TxGroup {
    pub id: TxGroupId,
    pub status: TransactionStatus,
    pub kind: TxGroupKind,
    pub tx_1: Option<TransactionId>,
    pub tx_2: Option<TransactionId>,
    pub tx_3: Option<TransactionId>,
    pub tx_4: Option<TransactionId>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub user_id: UserId,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "tx_groups"]
pub struct NewTxGroup {
    pub id: TxGroupId,
    pub status: TransactionStatus,
    pub kind: TxGroupKind,
    pub tx_1: Option<TransactionId>,
    pub tx_2: Option<TransactionId>,
    pub tx_3: Option<TransactionId>,
    pub tx_4: Option<TransactionId>,
}
