use chrono::NaiveDateTime;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::str::FromStr;

use diesel::deserialize::{self, FromSql};
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::VarChar;
use serde_json;

use models::*;
use prelude::*;
use repos::error::{Error as RepoError, ErrorContext as RepoErrorContex, ErrorKind as RepoErrorKind};
use schema::blockchain_transactions;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransactionEntryTo {
    pub address: BlockchainAddress,
    pub value: Amount,
}

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, Copy, Eq, PartialEq, Hash)]
#[sql_type = "VarChar"]
#[serde(rename_all = "snake_case")]
pub enum Erc20OperationKind {
    Approve,
    TransferFrom,
}

impl FromSql<VarChar, Pg> for Erc20OperationKind {
    fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
        match data {
            Some(b"approve") => Ok(Erc20OperationKind::Approve),
            Some(b"transfer_from") => Ok(Erc20OperationKind::TransferFrom),
            Some(v) => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8(v.to_vec()).unwrap_or_else(|_| "Non - UTF8 value".to_string())
            )
            .to_string()
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

impl ToSql<VarChar, Pg> for Erc20OperationKind {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
        match self {
            Erc20OperationKind::Approve => out.write_all(b"approve")?,
            Erc20OperationKind::TransferFrom => out.write_all(b"transfer_from")?,
        };
        Ok(IsNull::No)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransaction {
    pub hash: BlockchainTransactionId,
    pub from: Vec<BlockchainAddress>,
    pub to: Vec<BlockchainTransactionEntryTo>,
    pub block_number: u64,
    pub currency: Currency,
    pub fee: Amount,
    pub confirmations: usize,
    pub erc20_operation_kind: Option<Erc20OperationKind>,
}

impl BlockchainTransaction {
    pub fn unify_from_to(&self) -> Result<(HashSet<BlockchainAddress>, HashMap<BlockchainAddress, Amount>), RepoError> {
        //getting all from transactions to without repeats
        let from: HashSet<BlockchainAddress> = self.from.clone().into_iter().collect();

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

    pub fn normalized(&self) -> Option<BlockchainTransaction> {
        let from: HashSet<BlockchainAddress> = self.from.clone().into_iter().collect();

        //getting all to transactions to without repeats
        let mut to = HashMap::new();
        for x in self.to.clone() {
            let balance = to.entry(x.address).or_insert(Amount::new(0));
            if let Some(new_balance) = balance.checked_add(x.value) {
                *balance = new_balance;
            } else {
                return None;
            }
        }

        //deleting `from` from `to`
        for address in &from {
            to.remove(&address);
        }
        let mut from: Vec<_> = from.into_iter().collect();
        from.sort();
        let mut to: Vec<_> = to
            .into_iter()
            .map(|(address, value)| BlockchainTransactionEntryTo { address, value })
            .collect();
        to.sort_by_key(|entry| entry.address.clone());
        Some(BlockchainTransaction { from, to, ..self.clone() })
    }

    pub fn value(&self) -> Option<Amount> {
        self.to
            .iter()
            .fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|a| a.checked_add(elem.value)))
    }
}

#[derive(Debug, Queryable, Clone)]
pub struct BlockchainTransactionDB {
    pub hash: BlockchainTransactionId,
    pub block_number: i64,
    pub currency: Currency,
    pub fee: Amount,
    pub confirmations: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub from_: serde_json::Value,
    pub to_: serde_json::Value,
    pub erc20_operation_kind: Option<Erc20OperationKind>,
}

impl From<BlockchainTransaction> for NewBlockchainTransactionDB {
    fn from(transaction: BlockchainTransaction) -> Self {
        // Direct conversion of transaction.from to Value gives and `u128 not supported` error
        // This hack works, but you need to set arbitrary_precision feature for serde_json
        let from_str = serde_json::to_string(&transaction.from).unwrap();
        let from_ = serde_json::value::Value::from_str(&from_str).unwrap();
        let to_str = serde_json::to_string(&transaction.to).unwrap();
        let to_ = serde_json::value::Value::from_str(&to_str).unwrap();
        Self {
            hash: transaction.hash,
            from_,
            to_,
            block_number: transaction.block_number as i64,
            currency: transaction.currency,
            fee: transaction.fee,
            confirmations: transaction.confirmations as i32,
            erc20_operation_kind: transaction.erc20_operation_kind,
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
            fee: transaction.fee,
            confirmations: transaction.confirmations as usize,
            erc20_operation_kind: transaction.erc20_operation_kind,
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
    pub fee: Amount,
    pub confirmations: i32,
    pub erc20_operation_kind: Option<Erc20OperationKind>,
}

impl Default for NewBlockchainTransactionDB {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            from_: serde_json::Value::Array(vec![]),
            to_: serde_json::Value::Array(vec![]),
            block_number: 0,
            currency: Currency::Eth,
            fee: Amount::default(),
            confirmations: 0,
            erc20_operation_kind: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized() {
        let cases = [
            (vec!["1"], vec![("4", Amount::new(25))], vec!["1"], vec![("4", Amount::new(25))]),
            (
                vec!["1", "1", "2", "3", "2"],
                vec![
                    ("1", Amount::new(25)),
                    ("2", Amount::new(35)),
                    ("3", Amount::new(45)),
                    ("3", Amount::new(45)),
                    ("11", Amount::new(45)),
                    ("12", Amount::new(55)),
                    ("11", Amount::new(55)),
                    ("12", Amount::new(5)),
                    ("10", Amount::new(25)),
                ],
                vec!["1", "2", "3"],
                vec![("10", Amount::new(25)), ("11", Amount::new(100)), ("12", Amount::new(60))],
            ),
        ];
        for (from, to, from_res, to_res) in cases.into_iter() {
            let from: Vec<_> = from.into_iter().map(|x| BlockchainAddress::new(x.to_string())).collect();
            let from_res: Vec<_> = from_res.into_iter().map(|x| BlockchainAddress::new(x.to_string())).collect();
            let to: Vec<_> = to
                .into_iter()
                .map(|(address, value)| BlockchainTransactionEntryTo {
                    address: BlockchainAddress::new(address.to_string()),
                    value: *value,
                })
                .collect();
            let to_res: Vec<_> = to_res
                .into_iter()
                .map(|(address, value)| BlockchainTransactionEntryTo {
                    address: BlockchainAddress::new(address.to_string()),
                    value: *value,
                })
                .collect();

            let tx = BlockchainTransaction {
                from,
                to,
                ..Default::default()
            };
            let tx_res = BlockchainTransaction {
                from: from_res,
                to: to_res,
                hash: tx.hash.clone(),
                ..Default::default()
            };

            let normalized_tx = tx.normalized().unwrap();
            assert_eq!(normalized_tx, tx_res);
        }
    }

    #[test]
    fn test_value() {
        let tx = BlockchainTransaction {
            to: vec![
                BlockchainTransactionEntryTo {
                    value: Amount::new(30),
                    ..Default::default()
                },
                BlockchainTransactionEntryTo {
                    value: Amount::new(1),
                    ..Default::default()
                },
                BlockchainTransactionEntryTo {
                    value: Amount::new(2),
                    ..Default::default()
                },
                BlockchainTransactionEntryTo {
                    value: Amount::new(100),
                    ..Default::default()
                },
                BlockchainTransactionEntryTo {
                    value: Amount::new(5),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(tx.value(), Some(Amount::new(138)));

        let tx = BlockchainTransaction {
            to: vec![
                BlockchainTransactionEntryTo {
                    value: Amount::new(u128::max_value()),
                    ..Default::default()
                },
                BlockchainTransactionEntryTo {
                    value: Amount::new(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(tx.value(), None);
    }
}
