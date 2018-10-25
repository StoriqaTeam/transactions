use std::time::SystemTime;

use models::*;
use schema::pending_blockchain_transactions;

#[derive(Debug, Queryable, Clone)]
pub struct PendingBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: AccountAddress,
    pub to_: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<(CreateBlockchainTx, BlockchainTransactionId)> for NewPendingBlockchainTransactionDB {
    fn from(transaction: (CreateBlockchainTx, BlockchainTransactionId)) -> Self {
        Self {
            hash: transaction.1,
            from_: transaction.0.from,
            to_: transaction.0.to,
            currency: transaction.0.currency,
            value: transaction.0.value,
            fee: transaction.0.fee_price,
        }
    }
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "pending_blockchain_transactions"]
pub struct NewPendingBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: AccountAddress,
    pub to_: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
}

impl Default for NewPendingBlockchainTransactionDB {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            from_: AccountAddress::default(),
            to_: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
        }
    }
}
