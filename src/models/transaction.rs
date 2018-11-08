use chrono::NaiveDateTime;

use diesel::sql_types::Numeric;
use diesel::sql_types::Uuid as SqlUuid;
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
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub gid: TransactionId,
}

#[derive(Debug, Queryable, Clone, QueryableByName)]
pub struct TransactionSum {
    #[sql_type = "SqlUuid"]
    pub account_id: AccountId,
    #[sql_type = "Numeric"]
    pub sum: Amount,
}

impl Default for Transaction {
    fn default() -> Self {
        let id = TransactionId::generate();
        Self {
            id,
            gid: id,
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            blockchain_tx_id: None,
            created_at: ::chrono::Utc::now().naive_utc(),
            updated_at: ::chrono::Utc::now().naive_utc(),
        }
    }
}

#[derive(Debug, Insertable, Validate, Clone)]
#[table_name = "transactions"]
pub struct NewTransaction {
    pub id: TransactionId,
    pub gid: TransactionId,
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub cr_account_id: AccountId,
    pub currency: Currency,
    pub value: Amount,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
}

impl Default for NewTransaction {
    fn default() -> Self {
        let id = TransactionId::generate();
        Self {
            id,
            gid: id,
            user_id: UserId::generate(),
            dr_account_id: AccountId::generate(),
            cr_account_id: AccountId::generate(),
            currency: Currency::Stq,
            value: Amount::default(),
            blockchain_tx_id: None,
        }
    }
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransactionInput {
    pub id: TransactionId,
    pub user_id: UserId,
    pub from: AccountId,
    pub to: Receipt,
    pub to_type: ReceiptType,
    pub to_currency: Currency,
    pub value: Amount,
    pub value_currency: Currency,
    pub fee: Amount,
    pub exchange_id: Option<ExchangeId>,
    pub exchange_rate: Option<f64>,
}

#[derive(Debug, Clone, Validate)]
pub struct CreateTransaction {
    pub user_id: UserId,
    pub dr_account_id: AccountId,
    pub to: Receipt,
    pub to_type: ReceiptType,
    pub to_currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub hold_until: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Validate)]
pub struct DepositFunds {
    pub user_id: UserId,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub blockchain_tx_id: BlockchainTransactionId,
}

impl Default for DepositFunds {
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

#[derive(Debug, Clone, Validate)]
pub struct Withdraw {
    pub user_id: UserId,
    pub dr_account: Account,
    pub address: AccountAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
}

impl Withdraw {
    pub fn new(create: &CreateTransaction, dr_account: Account, address: AccountAddress) -> Self {
        Self {
            user_id: create.user_id,
            dr_account,
            address,
            currency: create.to_currency,
            value: create.value,
            fee: create.fee,
        }
    }
}

impl Default for Withdraw {
    fn default() -> Self {
        Self {
            user_id: UserId::default(),
            dr_account: Account::default(),
            address: AccountAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee: Amount::default(),
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BitcoinUtxos {
    tx_hash: BlockchainTransactionId,
    index: u64,
    value: Amount,
}

impl Default for BitcoinUtxos {
    fn default() -> Self {
        Self {
            tx_hash: BlockchainTransactionId::default(),
            index: 0,
            value: Amount::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransactionOut {
    pub id: TransactionId,
    pub from: Vec<TransactionAddressInfo>,
    pub to: TransactionAddressInfo,
    pub from_value: Amount,
    pub from_currency: Currency,
    pub to_value: Amount,
    pub to_currency: Currency,
    pub fee: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// impl TransactionOut {
//     pub fn new(transaction: &Transaction, from: Vec<TransactionAddressInfo>, to: TransactionAddressInfo) -> Self {
//         Self {
//             id: transaction.id,
//             from,
//             to,
//             currency: transaction.currency,
//             value: transaction.value,
//             fee: transaction.fee,
//             status: transaction.status,
//             blockchain_tx_id: transaction.blockchain_tx_id.clone(),
//             created_at: transaction.created_at.clone(),
//             updated_at: transaction.updated_at.clone(),
//         }
//     }
// }

#[derive(Debug, Serialize, Clone)]
pub struct TransactionAddressInfo {
    pub account_id: Option<AccountId>,
    pub blockchain_address: AccountAddress,
}

impl TransactionAddressInfo {
    pub fn new(account_id: Option<AccountId>, blockchain_address: AccountAddress) -> Self {
        Self {
            account_id,
            blockchain_address,
        }
    }
}
