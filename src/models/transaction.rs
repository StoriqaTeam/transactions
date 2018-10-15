use std::time::SystemTime;

use validator::Validate;

use models::*;
use schema::transactions;

#[derive(Debug, Queryable, Clone)]
pub struct Transaction {
    pub id: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub hold_until: Option<SystemTime>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            hold_until: None,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "transactions"]
pub struct NewTransaction {
    pub id: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub hold_until: Option<SystemTime>,
}

impl Default for NewTransaction {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            hold_until: None,
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransactionLocal {
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub hold_until: Option<SystemTime>,
}

impl Default for CreateTransactionLocal {
    fn default() -> Self {
        Self {
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Eth,
            value: Amount::default(),
            hold_until: None,
        }
    }
}

impl From<CreateTransactionLocal> for NewTransaction {
    fn from(create: CreateTransactionLocal) -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: create.user_id,
            dr_account_id: create.dr_account_id,
            cr_account_id: create.cr_account_id,
            currency: create.currency,
            value: create.value,
            hold_until: create.hold_until,
            status: TransactionStatus::Done,
            blockchain_tx_id: None,
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct DepositFounds {
    pub user_id: UserId,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub blockchain_tx_id: BlockchainTransactionId,
}

impl Default for DepositFounds {
    fn default() -> Self {
        Self {
            user_id: UserId::default(),
            address: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            blockchain_tx_id: BlockchainTransactionId::default(),
        }
    }
}

impl From<(DepositFounds, AccountId, AccountId)> for NewTransaction {
    fn from(create: (DepositFounds, AccountId, AccountId)) -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: create.0.user_id,
            currency: create.0.currency,
            value: create.0.value,
            hold_until: None,
            cr_account_id: create.1,
            dr_account_id: create.2,
            status: TransactionStatus::Done,
            blockchain_tx_id: Some(create.0.blockchain_tx_id),
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct Withdraw {
    pub user_id: UserId,
    pub account_id: AccountId,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
}

impl Default for Withdraw {
    fn default() -> Self {
        Self {
            user_id: UserId::default(),
            account_id: AccountId::generate(),
            address: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
        }
    }
}

impl From<(Withdraw, Amount, AccountId, BlockchainTransactionId)> for NewTransaction {
    fn from(create: (Withdraw, Amount, AccountId, BlockchainTransactionId)) -> Self {
        Self {
            id: TransactionId::generate(),
            user_id: create.0.user_id,
            currency: create.0.currency,
            value: create.1,
            cr_account_id: create.0.account_id,
            dr_account_id: create.2,
            hold_until: None,
            status: TransactionStatus::Pending,
            blockchain_tx_id: Some(create.3),
        }
    }
}

#[derive(Debug, Validate, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockchainTx {
    pub id: TransactionId,
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee_price: Amount,
    pub nonce: Option<u64>,
    pub utxos: Option<Vec<BitcoinUtxos>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinUtxos {
    tx_hash: BlockchainTransactionId,
    index: u64,
    value: Amount,
}

impl Default for CreateBlockchainTx {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            from: AccountAddress::default(),
            to: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee_price: Amount::default(),
            nonce: Some(0),
            utxos: None,
        }
    }
}

impl CreateBlockchainTx {
    pub fn new(
        from: AccountAddress,
        to: AccountAddress,
        currency: Currency,
        value: Amount,
        fee_price: Amount,
        nonce: Option<u64>,
        utxos: Option<Vec<BitcoinUtxos>>,
    ) -> Self {
        Self {
            id: TransactionId::generate(),
            from,
            to,
            currency,
            value,
            fee_price,
            nonce,
            utxos,
        }
    }
}
