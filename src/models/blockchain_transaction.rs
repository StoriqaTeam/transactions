use std::time::SystemTime;

use serde_json;

use models::*;
use schema::blockchain_transactions;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransactionEntry {
    pub address: AccountAddress,
    pub value: Amount,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransaction {
    pub hash: BlockchainTransactionId,
    pub from: Vec<BlockchainTransactionEntry>,
    pub to: Vec<BlockchainTransactionEntry>,
    pub block_number: u64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: usize,
}

#[derive(Debug, Queryable, Clone)]
pub struct BlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub block_number: i64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: i32,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub from_: serde_json::Value,
    pub to_: serde_json::Value,
}

impl From<BlockchainTransaction> for NewBlockchainTransactionDB {
    fn from(transaction: BlockchainTransaction) -> Self {
        Self {
            hash: transaction.hash,
            from_: serde_json::to_value(transaction.from).unwrap_or_default(),
            to_: serde_json::to_value(transaction.to).unwrap_or_default(),
            block_number: transaction.block_number as i64,
            currency: transaction.currency,
            value: transaction.value,
            fee: transaction.fee,
            confirmations: transaction.confirmations as i32,
        }
    }
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "blockchain_transactions"]
pub struct NewBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: serde_json::Value,
    pub to_: serde_json::Value,
    pub block_number: i64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: i32,
}

impl Default for NewBlockchainTransactionDB {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            from_: serde_json::Value::Array(vec![]),
            to_: serde_json::Value::Array(vec![]),
            block_number: 0,
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
            confirmations: 0,
        }
    }
}
