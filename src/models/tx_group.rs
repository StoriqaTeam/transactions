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

/// TxGroup is an entity that will represents transactions interface for the service api.
/// Underneath TxGroup there are actually several real transactions (database level).
/// `Base_tx`, `from_tx`, `to_tx`, `fee_tx`, `withdrawas_txs` are filled depending on TxGroup status
/// and kind:
///
/// 1) Deposit
///    Only base_tx is filled once deposit arrives
///
/// 2) Internal
///    Only base_tx is filled with the actual internal transaction (no fees involved)
///
/// 3) InternalMulti
///    from_tx and to_tx are filled, representing exchange with system accounts
///
/// 4) Withdrawal
///    At first step several transactions are created - a withdrawal acc at dr side of tx
///    and several different dr accounts, that will be subject to the actual withdrawal
///    (one account may have not enough funds). The status is set as Pending.
///
///    Once txs start to arrive from blockchain we switch status of each tx from Pending to Done and
///    add fees transactions.
/// 4) WithdrawalMulti
///    This basically the same as InternalMulti, but we do a withdrawal from system liquidity account

#[derive(Debug, Queryable, Clone, Identifiable)]
pub struct TxGroup {
    pub id: TxGroupId,
    pub status: TransactionStatus,
    pub kind: TxGroupKind,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub user_id: UserId,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub base_tx: Option<TransactionId>,
    pub from_tx: Option<TransactionId>,
    pub to_tx: Option<TransactionId>,
    pub fee_tx: Option<TransactionId>,
    // array of ids
    pub withdrawal_txs: serde_json::Value,
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "tx_groups"]
pub struct NewTxGroup {
    pub id: TxGroupId,
    pub status: TransactionStatus,
    pub kind: TxGroupKind,
    pub user_id: UserId,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub base_tx: Option<TransactionId>,
    pub from_tx: Option<TransactionId>,
    pub to_tx: Option<TransactionId>,
    pub fee_tx: Option<TransactionId>,
    pub withdrawal_txs: serde_json::Value,
}

#[derive(Debug, AsChangeset, Clone, Default)]
#[table_name = "tx_groups"]
pub struct UpdateTxGroup {
    pub status: Option<TransactionStatus>,
    pub kind: Option<TxGroupKind>,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub base_tx: Option<TransactionId>,
    pub from_tx: Option<TransactionId>,
    pub to_tx: Option<TransactionId>,
    pub fee_tx: Option<TransactionId>,
    pub withdrawal_txs: Option<serde_json::Value>,
}
