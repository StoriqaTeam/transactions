use std::time::SystemTime;

use models::*;
use schema::blockchain_transactions;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransaction {
    pub hash: BlockchainTransactionId,
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub block_number: u64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: usize,
}

#[derive(Debug, Queryable, Clone)]
pub struct BlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: AccountAddress,
    pub to_: AccountAddress,
    pub block_number: i64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: i32,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<BlockchainTransaction> for NewBlockchainTransactionDB {
    fn from(transaction: BlockchainTransaction) -> Self {
        Self {
            hash: transaction.hash,
            from_: transaction.from,
            to_: transaction.to,
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
    pub from_: AccountAddress,
    pub to_: AccountAddress,
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
            from_: AccountAddress::default(),
            to_: AccountAddress::default(),
            block_number: 0,
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
            confirmations: 0,
        }
    }
}
