use chrono::NaiveDateTime;

use diesel::sql_types::Numeric;
use diesel::sql_types::Uuid as SqlUuid;
use serde_json::Value;
use validator::{Validate, ValidationError};

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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub gid: TransactionId,
    pub kind: TransactionKind,
    pub group_kind: TransactionGroupKind,
    pub related_tx: Option<TransactionId>,
    pub meta: Value,
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
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            created_at: ::chrono::Utc::now().naive_utc(),
            updated_at: ::chrono::Utc::now().naive_utc(),
            kind: TransactionKind::Internal,
            group_kind: TransactionGroupKind::Internal,
            related_tx: None,
            meta: json!({}),
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
    pub status: TransactionStatus,
    pub blockchain_tx_id: Option<BlockchainTransactionId>,
    pub kind: TransactionKind,
    pub group_kind: TransactionGroupKind,
    pub related_tx: Option<TransactionId>,
    pub meta: Option<Value>,
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
            status: TransactionStatus::Pending,
            blockchain_tx_id: None,
            kind: TransactionKind::Internal,
            group_kind: TransactionGroupKind::Internal,
            related_tx: None,
            meta: None,
        }
    }
}

fn valid_rate(input: f64) -> Result<(), ValidationError> {
    if input > 0f64 {
        Ok(())
    } else {
        let mut error = ValidationError::new("le_zero");
        error.message = Some("Value is less or equal zero".into());
        error.add_param("value".into(), &input.to_string());
        Err(error)
    }
}

fn valid_exchange(input: &CreateTransactionInput) -> Result<(), ValidationError> {
    if input.exchange_id.is_some() {
        if input.exchange_rate.is_some() {
            Ok(())
        } else {
            let mut error = ValidationError::new("not_present");
            error.message = Some("Exchange id presents, but exchange rate doesn't".into());
            Err(error)
        }
    } else if input.exchange_rate.is_some() {
        let mut error = ValidationError::new("not_present");
        error.message = Some("Exchange rate presents, but exchange id doesn't".into());
        Err(error)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Validate)]
#[validate(schema(function = "valid_exchange", skip_on_field_errors = "false"))]
pub struct CreateTransactionInput {
    pub id: TransactionId,
    pub user_id: UserId,
    pub from: AccountId,
    pub to: Recepient,
    pub to_type: RecepientType,
    pub to_currency: Currency,
    pub value: Amount,
    pub value_currency: Currency,
    pub fee: Amount,
    pub exchange_id: Option<ExchangeId>,
    #[validate(custom = "valid_rate")]
    pub exchange_rate: Option<f64>,
}

#[derive(Debug, Validate, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockchainTx {
    pub id: TransactionId,
    pub from: BlockchainAddress,
    pub to: BlockchainAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee_price: f64,
    pub nonce: Option<u64>,
    pub utxos: Option<Vec<BitcoinUtxos>>,
}

impl Default for CreateBlockchainTx {
    fn default() -> Self {
        Self {
            id: TransactionId::generate(),
            from: BlockchainAddress::default(),
            to: BlockchainAddress::default(),
            currency: Currency::Eth,
            value: Amount::default(),
            fee_price: 0.0,
            nonce: Some(0),
            utxos: None,
        }
    }
}

impl CreateBlockchainTx {
    pub fn new(
        from: BlockchainAddress,
        to: BlockchainAddress,
        currency: Currency,
        value: Amount,
        fee_price: f64,
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

#[derive(Debug, Clone, Serialize)]
pub struct TransactionOut {
    pub id: TransactionId,
    pub user_id: UserId,
    pub from: Vec<TransactionAddressInfo>,
    pub to: TransactionAddressInfo,
    pub from_value: Amount,
    pub from_currency: Currency,
    pub to_value: Amount,
    pub to_currency: Currency,
    pub fee: Amount,
    pub status: TransactionStatus,
    pub blockchain_tx_ids: Vec<BlockchainTransactionId>,
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
    pub blockchain_address: BlockchainAddress,
}
