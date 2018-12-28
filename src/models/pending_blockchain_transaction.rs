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

impl From<PendingBlockchainTransactionDB> for BlockchainTransaction {
    fn from(transaction: PendingBlockchainTransactionDB) -> Self {
        Self {
            hash: transaction.hash,
            from: vec![transaction.from_],
            to: vec![BlockchainTransactionEntryTo {
                address: transaction.to_,
                value: transaction.value,
            }],
            block_number: 0,
            currency: transaction.currency,
            fee: transaction.fee,
            confirmations: 0 as usize,
            erc20_operation_kind: transaction.erc20_operation_kind,
        }
    }
}

impl From<(CreateBlockchainTx, BlockchainTransactionId)> for NewPendingBlockchainTransactionDB {
    fn from(transaction: (CreateBlockchainTx, BlockchainTransactionId)) -> Self {
        Self {
            hash: transaction.1,
            from_: transaction.0.from,
            to_: transaction.0.to,
            currency: transaction.0.currency,
            value: transaction.0.value,
            fee: Amount::new(0),
            erc20_operation_kind: None,
        }
    }
}

impl From<(ApproveInput, BlockchainTransactionId)> for NewPendingBlockchainTransactionDB {
    fn from(transaction: (ApproveInput, BlockchainTransactionId)) -> Self {
        Self {
            hash: transaction.1,
            from_: transaction.0.address,
            to_: transaction.0.approve_address,
            currency: transaction.0.currency,
            value: transaction.0.value,
            fee: Amount::new(0),
            erc20_operation_kind: Some(Erc20OperationKind::Approve),
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

impl From<PendingBlockchainTransactionDB> for NewBlockchainTransactionDB {
    fn from(transaction: PendingBlockchainTransactionDB) -> Self {
        let bl: BlockchainTransaction = transaction.into();
        bl.into()
    }
}
