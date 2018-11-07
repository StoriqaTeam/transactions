use chrono::NaiveDateTime;

use serde_json;

use models::*;
use schema::strange_blockchain_transactions;

#[derive(Debug, Queryable, Clone)]
pub struct StrangeBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: serde_json::Value,
    pub to_: serde_json::Value,
    pub block_number: i64,
    pub currency: Currency,
    pub fee: Amount,
    pub confirmations: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub commentary: String,
}

impl From<(BlockchainTransaction, String)> for NewStrangeBlockchainTransactionDB {
    fn from(transaction: (BlockchainTransaction, String)) -> Self {
        Self {
            hash: transaction.0.hash,
            from_: serde_json::to_value(transaction.0.from).unwrap_or_default(),
            to_: serde_json::to_value(transaction.0.to).unwrap_or_default(),
            block_number: transaction.0.block_number as i64,
            currency: transaction.0.currency,
            fee: transaction.0.fee,
            confirmations: transaction.0.confirmations as i32,
            commentary: transaction.1,
        }
    }
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "strange_blockchain_transactions"]
pub struct NewStrangeBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: serde_json::Value,
    pub to_: serde_json::Value,
    pub block_number: i64,
    pub currency: Currency,
    pub fee: Amount,
    pub confirmations: i32,
    pub commentary: String,
}

impl Default for NewStrangeBlockchainTransactionDB {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            from_: serde_json::Value::Array(vec![]),
            to_: serde_json::Value::Array(vec![]),
            block_number: 0,
            currency: Currency::Eth,
            fee: Amount::default(),
            confirmations: 0,
            commentary: "comment".to_string(),
        }
    }
}
