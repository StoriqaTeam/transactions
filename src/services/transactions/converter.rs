use std::sync::Arc;

use super::super::error::*;
use super::super::system::*;
use models::*;
use prelude::*;
use repos::{AccountsRepo, BlockchainTransactionsRepo, PendingBlockchainTransactionsRepo};

pub trait ConverterService: Send + Sync + 'static {
    fn convert_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error>;
}

#[derive(Clone)]
pub struct ConverterServiceImpl {
    accounts_repo: Arc<AccountsRepo>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    system_service: Arc<SystemService>,
}

impl ConverterServiceImpl {
    pub fn new(
        accounts_repo: Arc<AccountsRepo>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        system_service: Arc<SystemService>,
    ) -> Self {
        Self {
            accounts_repo,
            pending_blockchain_transactions_repo,
            blockchain_transactions_repo,
            system_service,
        }
    }

    // 1) Deposit
    //   Always 1 tx with status Done
    fn convert_deposit_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        if transactions.len() != 1 {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let tx = transactions[0].clone();
        if tx.kind != TransactionKind::Deposit {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        if tx.blockchain_tx_id.is_none() {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let blockchain_tx_id = tx.blockchain_tx_id.clone().unwrap();
        let blockchain_tx = self
            .blockchain_transactions_repo
            .get(blockchain_tx_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => blockchain_tx_id))?;

        if blockchain_tx.is_none() {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }

        let blockchain_tx = Into::<BlockchainTransaction>::into(blockchain_tx.unwrap())
            .normalized()
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let from: Vec<_> = blockchain_tx
            .from
            .into_iter()
            .map(|blockchain_address| TransactionAddressInfo {
                account_id: None,
                blockchain_address,
            })
            .collect();
        let to_acct_id = tx.cr_account_id.clone();
        let to_account = self
            .accounts_repo
            .get(to_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => to_acct_id))?;
        if to_account.is_none() {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let to_account = to_account.unwrap();
        let to = TransactionAddressInfo {
            account_id: Some(tx.cr_account_id),
            blockchain_address: to_account.address,
        };
        Ok(TransactionOut {
            id: tx.gid,
            user_id: to_account.user_id,
            from,
            to,
            from_value: tx.value,
            from_currency: tx.currency,
            to_value: tx.value,
            to_currency: tx.currency,
            fee: Amount::new(0),
            status: tx.status,
            blockchain_tx_ids: tx.blockchain_tx_id.iter().cloned().collect(),
            created_at: tx.created_at,
            updated_at: tx.updated_at,
        })
    }

    // 2) Internal:
    //   Always 1 tx with status Done
    fn convert_internal_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        if transactions.len() != 1 {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let tx = transactions[0].clone();
        if tx.kind != TransactionKind::Internal {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }

        let from_acct_id = tx.dr_account_id.clone();
        let from_account = self
            .accounts_repo
            .get(from_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => from_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => from_acct_id))?;

        let to_acct_id = tx.cr_account_id.clone();
        let to_account = self
            .accounts_repo
            .get(to_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => to_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => to_acct_id))?;

        let from = vec![TransactionAddressInfo {
            account_id: Some(from_account.id),
            blockchain_address: from_account.address,
        }];
        let to = TransactionAddressInfo {
            account_id: Some(to_account.id),
            blockchain_address: to_account.address,
        };

        Ok(TransactionOut {
            id: tx.gid,
            from,
            to,
            from_value: tx.value,
            from_currency: tx.currency,
            to_value: tx.value,
            to_currency: tx.currency,
            fee: Amount::new(0),
            status: tx.status,
            blockchain_tx_ids: tx.blockchain_tx_id.iter().cloned().collect(),
            created_at: tx.created_at,
            updated_at: tx.updated_at,
        })
    }

    // 6) Internal:
    //   Always 1 tx with status Done
    fn convert_reversal_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        if transactions.len() != 1 {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let tx = transactions[0].clone();
        if tx.kind != TransactionKind::Reversal {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }

        let from_acct_id = tx.dr_account_id.clone();
        let from_account = self
            .accounts_repo
            .get(from_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => from_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => from_acct_id))?;

        let to_acct_id = tx.cr_account_id.clone();
        let to_account = self
            .accounts_repo
            .get(to_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => to_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => to_acct_id))?;

        let from = vec![TransactionAddressInfo {
            account_id: Some(from_account.id),
            blockchain_address: from_account.address,
        }];
        let to = TransactionAddressInfo {
            account_id: Some(to_account.id),
            blockchain_address: to_account.address,
        };

        Ok(TransactionOut {
            id: tx.gid,
            user_id: to_account.user_id,
            from,
            to,
            from_value: tx.value,
            from_currency: tx.currency,
            to_value: tx.value,
            to_currency: tx.currency,
            fee: Amount::new(0),
            status: tx.status,
            blockchain_tx_ids: tx.blockchain_tx_id.iter().cloned().collect(),
            created_at: tx.created_at,
            updated_at: tx.updated_at,
        })
    }

    // 4) InternalMulti:
    //   two txs: MultiFrom - Done, MultiTo - Done
    fn convert_internal_multi_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        if transactions.len() != 2 {
            return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
        }
        let from_tx = transactions
            .iter()
            .find(|tx| tx.kind == TransactionKind::MultiFrom)
            .cloned()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let to_tx = transactions
            .iter()
            .find(|tx| tx.kind == TransactionKind::MultiTo)
            .cloned()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;

        let from_acct_id = from_tx.dr_account_id.clone();
        let from_account = self
            .accounts_repo
            .get(from_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => from_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => from_acct_id))?;

        let to_acct_id = to_tx.cr_account_id.clone();
        let to_account = self
            .accounts_repo
            .get(to_acct_id.clone())
            .map_err(ectx!(try ErrorKind::Internal => to_acct_id))?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => to_acct_id))?;

        let from = vec![TransactionAddressInfo {
            account_id: Some(from_account.id),
            blockchain_address: from_account.address,
        }];
        let to = TransactionAddressInfo {
            account_id: Some(to_account.id),
            blockchain_address: to_account.address,
        };
        Ok(TransactionOut {
            id: from_tx.gid,
            user_id: to_account.user_id,
            from,
            to,
            from_value: from_tx.value,
            from_currency: from_tx.currency,
            to_value: to_tx.value,
            to_currency: to_tx.currency,
            fee: Amount::new(0),
            status: TransactionStatus::Done,
            blockchain_tx_ids: vec![],
            created_at: from_tx.created_at,
            updated_at: from_tx.updated_at,
        })
    }

    // 3) Withdrawal:
    //   a) two txs: Withdrawal - Pending, Fee - Done
    //   b) three txs: Withdrwal - Done, Fee - Done, BlockchainFee - Done

    fn convert_external_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        let fee_tx = transactions
            .iter()
            .find(|tx| tx.kind == TransactionKind::Fee)
            .cloned()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        // We take arbitrary first withdrawal tx to extract some data
        let withdrawal_tx = transactions
            .iter()
            .find(|tx| tx.kind == TransactionKind::Withdrawal)
            .cloned()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let blockchain_tx_hash = withdrawal_tx
            .blockchain_tx_id
            .clone()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let withdrawal_account = self
            .accounts_repo
            .get(withdrawal_tx.dr_account_id)?
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        // Here the problem is it can be in pending txs as well
        let blockchain_tx_hash_clone = blockchain_tx_hash.clone();
        let blockchain_tx: BlockchainTransaction = self
            .blockchain_transactions_repo
            .get(blockchain_tx_hash_clone.clone())
            .map_err(ectx!(try ErrorKind::Internal => blockchain_tx_hash_clone))?
            .map(Into::<BlockchainTransaction>::into)
            .or(self
                .pending_blockchain_transactions_repo
                .get(blockchain_tx_hash.clone())
                .map_err(ectx!(try ErrorKind::Internal => blockchain_tx_hash))?
                .map(Into::<BlockchainTransaction>::into))
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let blockchain_tx = blockchain_tx
            .normalized()
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let to_address = blockchain_tx
            .to
            .get(0)
            .map(|entry| entry.address.clone())
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let from = vec![TransactionAddressInfo {
            account_id: Some(withdrawal_account.id),
            blockchain_address: withdrawal_account.address,
        }];
        let to = TransactionAddressInfo {
            account_id: None,
            blockchain_address: to_address,
        };
        // now get aggregates
        let withdrawal_txs: Vec<_> = transactions
            .iter()
            .filter(|tx| tx.kind == TransactionKind::Withdrawal)
            .cloned()
            .collect();

        let status = if withdrawal_txs.iter().all(|tx| tx.status == TransactionStatus::Done) {
            TransactionStatus::Done
        } else {
            TransactionStatus::Pending
        };
        let created_at = withdrawal_txs
            .iter()
            .map(|tx| tx.created_at)
            .min()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let updated_at = withdrawal_txs
            .iter()
            .map(|tx| tx.updated_at)
            .max()
            .ok_or(ectx!(try err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))?;
        let value = withdrawal_txs
            .iter()
            .fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|a| a.checked_add(elem.value)))
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => transactions))?;
        let blockchain_tx_ids: Vec<_> = withdrawal_txs.into_iter().flat_map(|tx| tx.blockchain_tx_id.into_iter()).collect();
        Ok(TransactionOut {
            id: withdrawal_tx.gid,
            user_id: withdrawal_account.user_id,
            from,
            to,
            from_value: value,
            from_currency: withdrawal_tx.currency,
            to_value: value,
            to_currency: withdrawal_tx.currency,
            fee: fee_tx.value,
            status,
            blockchain_tx_ids,
            created_at,
            updated_at,
        })
    }

    // 5) ExternalMulti:
    //   a) MultiFrom - Done, MultiTo - Done, Withdrawal - Pending, Fee - Done
    //   b) MultiFrom - Done, MultiTo - Done, Withdrawal - Done, Fee - Done, BlockchainFee - Done
    fn convert_external_multi_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        let currency_txs: Vec<_> = transactions
            .iter()
            .filter(|tx| (tx.kind == TransactionKind::MultiFrom) || (tx.kind == TransactionKind::MultiTo))
            .cloned()
            .collect();
        let withdrawal_txs: Vec<_> = transactions
            .iter()
            .filter(|tx| (tx.kind != TransactionKind::MultiFrom) && (tx.kind != TransactionKind::MultiTo))
            .cloned()
            .collect();
        let currency_tx_out = self.convert_internal_multi_transaction(currency_txs)?;
        let withdrawal_tx_out = self.convert_external_transaction(withdrawal_txs)?;
        Ok(TransactionOut {
            id: currency_tx_out.id,
            user_id: withdrawal_tx_out.user_id,
            from: currency_tx_out.from,
            to: withdrawal_tx_out.to,
            from_value: currency_tx_out.from_value,
            from_currency: currency_tx_out.from_currency,
            to_value: currency_tx_out.to_value,
            to_currency: currency_tx_out.to_currency,
            fee: withdrawal_tx_out.fee,
            status: withdrawal_tx_out.status,
            blockchain_tx_ids: withdrawal_tx_out.blockchain_tx_ids,
            created_at: withdrawal_tx_out.created_at,
            updated_at: withdrawal_tx_out.updated_at,
        })
    }
}

impl ConverterService for ConverterServiceImpl {
    // Cases are:
    //
    // 1) Deposit
    //   Always 1 tx with status Done
    //
    // 2) Internal:
    //   Always 1 tx with status Done
    //
    // 3) Withdrawal:
    //   a) two txs: Withdrawal - Pending, Fee - Done
    //   b) three txs: Withdrwal - Done, Fee - Done, BlockchainFee - Done
    //
    // 4) InternalMulti:
    //   two txs: MultiFrom - Done, MultiTo - Done
    //
    // 5) ExternalMulti:
    //   a) MultiFrom - Done, MultiTo - Done, Withdrawal - Pending, Fee - Done
    //   b) MultiFrom - Done, MultiTo - Done, Withdrawal - Done, Fee - Done, BlockchainFee - Done

    // 6) Approval - we don't serve this as TransactionOut since it's internal to our system
    // 7) Reversal
    //   Always 1 tx with status Done

    // Input txs should be with len() > 0 and have the same `gid`- this guarantees exactly one TransactionOut
    fn convert_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        let gid = transactions[0].gid;
        for tx in transactions.iter() {
            if gid != tx.gid {
                return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions));
            }
        }
        let group_kind = transactions[0].group_kind;
        match group_kind {
            TransactionGroupKind::Deposit => self.convert_deposit_transaction(transactions),
            TransactionGroupKind::Internal => self.convert_internal_transaction(transactions),
            TransactionGroupKind::InternalMulti => self.convert_internal_multi_transaction(transactions),
            TransactionGroupKind::Withdrawal => self.convert_external_transaction(transactions),
            TransactionGroupKind::WithdrawalMulti => self.convert_external_multi_transaction(transactions),
            TransactionGroupKind::Reversal => self.convert_reversal_transaction(transactions),
            TransactionGroupKind::Approval => {
                return Err(ectx!(err ErrorContext::InvalidTransactionStructure, ErrorKind::Internal => transactions))
            }
        }
        // // internal + withdrawal tx
        // if transactions.len() == 1 {
        //     let tx = transactions[0].clone();
        //     let (from_addrs, to_addr) = self.extract_address_info(tx.clone())?;
        //     return Ok(TransactionOut {
        //         id: tx.id,
        //         from: from_addrs,
        //         to: to_addr,
        //         from_value: tx.value,
        //         from_currency: tx.currency,
        //         to_value: tx.value,
        //         to_currency: tx.currency,
        //         fee: Amount::new(0),
        //         status: tx.status,
        //         blockchain_tx_id: tx.blockchain_tx_id,
        //         created_at: tx.created_at,
        //         updated_at: tx.updated_at,
        //     });
        // }
        // // internal multicurrency tx
        // if transactions.len() == 2 {
        //     let system_acc_id0 = self.system_service.get_system_liquidity_account(transactions[0].currency)?.id;
        //     let system_acc_id1 = self.system_service.get_system_liquidity_account(transactions[1].currency)?.id;
        //     let (from_tx, to_tx) = if transactions[0].cr_account_id == system_acc_id0 {
        //         assert_eq!(
        //             transactions[1].dr_account_id, system_acc_id1,
        //             "Inconsistency in exchange currencies: {:#?}",
        //             transactions
        //         );
        //         (transactions[0].clone(), transactions[1].clone())
        //     } else if transactions[0].dr_account_id == system_acc_id0 {
        //         assert_eq!(
        //             transactions[1].cr_account_id, system_acc_id1,
        //             "Inconsistency in exchange currencies: {:#?}",
        //             transactions
        //         );
        //         (transactions[1].clone(), transactions[0].clone())
        //     } else {
        //         panic!("Unexpected transactions sequence for multicurrency tx: {:#?}", transactions)
        //     };
        //     let (from_addrs, _) = self.extract_address_info(from_tx.clone())?;
        //     let (_, to_addr) = self.extract_address_info(to_tx.clone())?;
        //     return Ok(TransactionOut {
        //         id: from_tx.id,
        //         from: from_addrs,
        //         to: to_addr,
        //         from_value: from_tx.value,
        //         from_currency: from_tx.currency,
        //         to_value: to_tx.value,
        //         to_currency: to_tx.currency,
        //         fee: Amount::new(0),
        //         // Todo
        //         status: from_tx.status,
        //         blockchain_tx_id: to_tx.blockchain_tx_id,
        //         created_at: from_tx.created_at,
        //         updated_at: from_tx.updated_at,
        //     });
        // }
        // panic!("Unsupported transactions sequence: {:#?}", transactions)
    }
}
