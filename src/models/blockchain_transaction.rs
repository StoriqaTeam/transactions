use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::SystemTime;

use serde_json;

use models::*;
use prelude::*;
use repos::error::{Error as RepoError, ErrorContext as RepoErrorContex, ErrorKind as RepoErrorKind};
use schema::blockchain_transactions;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransactionEntryTo {
    pub address: AccountAddress,
    pub value: Amount,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransaction {
    pub hash: BlockchainTransactionId,
    pub from: Vec<AccountAddress>,
    pub to: Vec<BlockchainTransactionEntryTo>,
    pub block_number: u64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: usize,
}

impl BlockchainTransaction {
    pub fn unify_from_to(&self) -> Result<(HashSet<AccountAddress>, HashMap<AccountAddress, Amount>), RepoError> {
        //getting all from transactions to without repeats
        let from: HashSet<AccountAddress> = self.from.clone().into_iter().collect();

        //getting all to transactions to without repeats
        let mut to = HashMap::new();
        for x in self.to.clone() {
            let balance = to.entry(x.address).or_insert_with(Amount::default);
            if let Some(new_balance) = balance.checked_add(x.value) {
                *balance = new_balance;
            } else {
                return Err(ectx!(err RepoErrorContex::BalanceOverflow, RepoErrorKind::Internal => balance, x.value));
            }
        }

        //deleting `from` from `to`
        for address in &from {
            to.remove(&address);
        }
        Ok((from, to))
    }
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
        // Direct conversion of transaction.from to Value gives and `u128 not supported` error
        // This hack works, but you need to set arbitrary_precision feature for serde_json
        let from_str = serde_json::to_string(&transaction.from).unwrap();
        let from_ = serde_json::value::Value::from_str(&from_str).unwrap();
        let to_str = serde_json::to_string(&transaction.from).unwrap();
        let to_ = serde_json::value::Value::from_str(&to_str).unwrap();
        Self {
            hash: transaction.hash,
            from_,
            to_,
            block_number: transaction.block_number as i64,
            currency: transaction.currency,
            value: transaction.value,
            fee: transaction.fee,
            confirmations: transaction.confirmations as i32,
        }
    }
}

impl From<BlockchainTransactionDB> for BlockchainTransaction {
    fn from(transaction: BlockchainTransactionDB) -> Self {
        Self {
            hash: transaction.hash,
            from: serde_json::from_value(transaction.from_).unwrap_or_default(),
            to: serde_json::from_value(transaction.to_).unwrap_or_default(),
            block_number: transaction.block_number as u64,
            currency: transaction.currency,
            value: transaction.value,
            fee: transaction.fee,
            confirmations: transaction.confirmations as usize,
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
