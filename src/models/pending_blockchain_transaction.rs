use chrono::NaiveDateTime;

use models::*;
use schema::pending_blockchain_transactions;

#[derive(Debug, Queryable, Clone)]
pub struct PendingBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: BlockchainAddress,
    pub to_: BlockchainAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub erc20_operation_kind: Option<Erc20OperationKind>,
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
            erc20_operation_kind: None,
        }
    }
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "pending_blockchain_transactions"]
pub struct NewPendingBlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub from_: BlockchainAddress,
    pub to_: BlockchainAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub erc20_operation_kind: Option<Erc20OperationKind>,
}

impl Default for NewPendingBlockchainTransactionDB {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            from_: BlockchainAddress::default(),
            to_: BlockchainAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
            erc20_operation_kind: None,
        }
    }
}
