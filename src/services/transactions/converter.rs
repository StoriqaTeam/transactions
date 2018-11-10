use std::sync::Arc;

use super::super::error::*;
use super::system::*;
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

    fn extract_address_info(&self, transaction: Transaction) -> Result<(Vec<TransactionAddressInfo>, TransactionAddressInfo), Error> {
        let accounts_repo = self.accounts_repo.clone();
        let pending_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        let transaction_id = transaction.id;
        let cr_account = accounts_repo
            .get(transaction.cr_account_id)
            .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
        let cr_account_id = transaction.cr_account_id;
        let cr_account = cr_account.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => cr_account_id))?;

        let dr_account = accounts_repo
            .get(transaction.dr_account_id)
            .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
        let dr_account_id = transaction.dr_account_id;
        let dr_account = dr_account.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => dr_account_id))?;

        if cr_account.kind == AccountKind::Cr && dr_account.kind == AccountKind::Cr {
            let from = TransactionAddressInfo::new(Some(dr_account.id), dr_account.address);
            let to = TransactionAddressInfo::new(Some(cr_account.id), cr_account.address);
            Ok((vec![from], to))
        } else if cr_account.kind == AccountKind::Cr && dr_account.kind == AccountKind::Dr {
            let hash = transaction
                .blockchain_tx_id
                .clone()
                .ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => transaction_id))?;
            let to = TransactionAddressInfo::new(Some(cr_account.id), cr_account.address);

            let hash_clone = hash.clone();
            let hash_clone2 = hash.clone();
            let hash_clone3 = hash.clone();
            if let Some(pending_transaction) = pending_transactions_repo
                .get(hash.clone())
                .map_err(ectx!(try convert => hash_clone))?
            {
                let from = TransactionAddressInfo::new(None, pending_transaction.from_);
                Ok((vec![from], to))
            } else if let Some(blockchain_transaction_db) = blockchain_transactions_repo
                .get(hash.clone())
                .map_err(ectx!(try convert => hash_clone2))?
            {
                let blockchain_transaction: BlockchainTransaction = blockchain_transaction_db.into();
                let (froms, _) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert => hash))?;
                let from = froms
                    .into_iter()
                    .map(|address| TransactionAddressInfo::new(None, address))
                    .collect();
                Ok((from, to))
            } else {
                return Err(ectx!(err ErrorContext::NoTransaction, ErrorKind::NotFound => hash_clone3));
            }
        } else if cr_account.kind == AccountKind::Dr && dr_account.kind == AccountKind::Cr {
            let hash = transaction
                .blockchain_tx_id
                .clone()
                .ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => transaction_id))?;
            let from = TransactionAddressInfo::new(Some(dr_account.id), dr_account.address);

            let hash_clone = hash.clone();
            let hash_clone2 = hash.clone();
            let hash_clone3 = hash.clone();
            if let Some(pending_transaction) = pending_transactions_repo
                .get(hash.clone())
                .map_err(ectx!(try convert => hash_clone))?
            {
                let to = TransactionAddressInfo::new(None, pending_transaction.to_);
                Ok((vec![from], to))
            } else if let Some(blockchain_transaction_db) = blockchain_transactions_repo
                .get(hash.clone())
                .map_err(ectx!(try convert => hash_clone2))?
            {
                let hash_clone4 = hash.clone();
                let blockchain_transaction: BlockchainTransaction = blockchain_transaction_db.into();
                let (_, to_s) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert => hash_clone4))?;
                let to = to_s
                    .into_iter()
                    .map(|(address, _)| TransactionAddressInfo::new(None, address))
                    .nth(0);
                let to = to.ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => hash))?;
                Ok((vec![from], to))
            } else {
                return Err(ectx!(err ErrorContext::NoTransaction, ErrorKind::NotFound => hash_clone3));
            }
        } else {
            return Err(ectx!(err ErrorContext::InvalidTransaction, ErrorKind::Internal => transaction_id));
        }
    }
}

impl ConverterService for ConverterServiceImpl {
    // Input txs should be with len() > 0 and have the same `gid`- this guarantees exactly one TransactionOut
    fn convert_transaction(&self, transactions: Vec<Transaction>) -> Result<TransactionOut, Error> {
        let gid = transactions[0].gid;
        for tx in transactions.iter() {
            assert_eq!(gid, tx.gid, "Transaction gids doesn't match: {:#?}", transactions);
        }
        // internal + withdrawal tx
        if transactions.len() == 1 {
            let tx = transactions[0].clone();
            let (from_addrs, to_addr) = self.extract_address_info(tx.clone())?;
            return Ok(TransactionOut {
                id: tx.id,
                from: from_addrs,
                to: to_addr,
                from_value: tx.value,
                from_currency: tx.currency,
                to_value: tx.value,
                to_currency: tx.currency,
                fee: Amount::new(0),
                status: tx.status,
                blockchain_tx_id: tx.blockchain_tx_id,
                created_at: tx.created_at,
                updated_at: tx.updated_at,
            });
        }
        // internal multicurrency tx
        if transactions.len() == 2 {
            let system_acc_id0 = self.system_service.get_system_liquidity_account(transactions[0].currency)?.id;
            let system_acc_id1 = self.system_service.get_system_liquidity_account(transactions[1].currency)?.id;
            let (from_tx, to_tx) = if transactions[0].cr_account_id == system_acc_id0 {
                assert_eq!(
                    transactions[1].dr_account_id, system_acc_id1,
                    "Inconsistency in exchange currencies: {:#?}",
                    transactions
                );
                (transactions[0].clone(), transactions[1].clone())
            } else if transactions[0].dr_account_id == system_acc_id0 {
                assert_eq!(
                    transactions[1].cr_account_id, system_acc_id1,
                    "Inconsistency in exchange currencies: {:#?}",
                    transactions
                );
                (transactions[1].clone(), transactions[0].clone())
            } else {
                panic!("Unexpected transactions sequence for multicurrency tx: {:#?}", transactions)
            };
            let (from_addrs, _) = self.extract_address_info(from_tx.clone())?;
            let (_, to_addr) = self.extract_address_info(to_tx.clone())?;
            return Ok(TransactionOut {
                id: from_tx.id,
                from: from_addrs,
                to: to_addr,
                from_value: from_tx.value,
                from_currency: from_tx.currency,
                to_value: to_tx.value,
                to_currency: to_tx.currency,
                fee: Amount::new(0),
                // Todo
                status: from_tx.status,
                blockchain_tx_id: to_tx.blockchain_tx_id,
                created_at: from_tx.created_at,
                updated_at: from_tx.updated_at,
            });
        }
        panic!("Unsupported transactions sequence: {:#?}", transactions)
    }
}
