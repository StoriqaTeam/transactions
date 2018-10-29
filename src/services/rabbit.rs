use std::sync::Arc;

use super::error::*;
use models::*;
use prelude::*;
use repos::error::{Error as RepoError, ErrorContext as RepoErrorContex, ErrorKind as RepoErrorKind};
use repos::{
    AccountsRepo, BlockchainTransactionsRepo, DbExecutor, PendingBlockchainTransactionsRepo, SeenHashesRepo,
    StrangeBlockchainTransactionsRepo, TransactionsRepo,
};
use serde_json;

pub const ETHERIUM_PRICE: u128 = 200; // 200$, price of 1 eth in gwei
pub const STQ_PRICE: f64 = 0.0025; // 0,0025$, price of 1 stq in gwei
pub const WEI: u128 = 1_000_000_000_000_000_000;
pub const BITCOIN_PRICE: u128 = 6400; // 6400$ price in satoshi
pub const SATOSHI: u128 = 100_000_000;

#[derive(Clone)]
pub struct BlockchainFetcher<E: DbExecutor> {
    transactions_repo: Arc<TransactionsRepo>,
    accounts_repo: Arc<AccountsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn new(
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<AccountsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        db_executor: E,
    ) -> Self {
        BlockchainFetcher {
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
            strange_blockchain_transactions_repo,
            pending_blockchain_transactions_repo,
            db_executor,
        }
    }
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn process(&self, data: Vec<u8>) -> impl Future<Item = (), Error = Error> {
        let data_clone = data.clone();
        let transactions_repo = self.transactions_repo.clone();
        let seen_hashes_repo = self.seen_hashes_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        let strange_blockchain_transactions_repo = self.strange_blockchain_transactions_repo.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            String::from_utf8(data.clone())
                .map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => data_clone))
                .into_future()
                .and_then(|s| serde_json::from_str(&s).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => s)))
                .and_then(move |blockchain_transaction: BlockchainTransaction| {
                    db_executor
                        .execute_transaction(move || -> Result<(), RepoError> {
                            let total_value = blockchain_transaction.to.iter().try_fold(Amount::default(),|acc, x| {
                                acc.checked_add(x.value).ok_or_else(|| ectx!(err RepoErrorContex::BalanceOverflow, RepoErrorKind::Internal => x.value)) as Result<Amount, RepoError>
                            })?;
                            let enough_confirms = match blockchain_transaction.currency {
                                // # Ethereum
                                // < $20 / $200 - 0 conf
                                // < $50 / $200 - 1 conf
                                // < $200 / $200 - 2 conf
                                // < $500 / $200 - 3 conf
                                // < $1000 / $200 - 4 conf
                                // < $2000 / $200 - 5 conf
                                // < $3000 / $200 - 6 conf
                                // < $5000 / $200 - 8 conf
                                // > $5000 / $200 - 12 conf
                                Currency::Stq => match total_value.raw() {
                                    x if x < (((20f64 / STQ_PRICE) as u128) * WEI) => 0,
                                    x if x < (((50f64 / STQ_PRICE) as u128) * WEI) => 1,
                                    x if x < (((200f64 / STQ_PRICE) as u128) * WEI) => 2,
                                    x if x < (((500f64 / STQ_PRICE) as u128) * WEI) => 3,
                                    x if x < (((1000f64 / STQ_PRICE) as u128) * WEI) => 4,
                                    x if x < (((2000f64 / STQ_PRICE) as u128) * WEI) => 5,
                                    x if x < (((3000f64 / STQ_PRICE) as u128) * WEI) => 6,
                                    x if x < (((5000f64 / STQ_PRICE) as u128) * WEI) => 8,
                                    _ => 12,
                                },
                                // # Ethereum
                                // < $20 / $200 - 0 conf
                                // < $50 / $200 - 1 conf
                                // < $200 / $200 - 2 conf
                                // < $500 / $200 - 3 conf
                                // < $1000 / $200 - 4 conf
                                // < $2000 / $200 - 5 conf
                                // < $3000 / $200 - 6 conf
                                // < $5000 / $200 - 8 conf
                                // > $5000 / $200 - 12 conf
                                Currency::Eth => match total_value.raw() {
                                    x if x < (20 * WEI / ETHERIUM_PRICE) => 0,
                                    x if x < (50 * WEI / ETHERIUM_PRICE) => 1,
                                    x if x < (200 * WEI / ETHERIUM_PRICE) => 2,
                                    x if x < (500 * WEI / ETHERIUM_PRICE) => 3,
                                    x if x < (1000 * WEI / ETHERIUM_PRICE) => 4,
                                    x if x < (2000 * WEI / ETHERIUM_PRICE) => 5,
                                    x if x < (3000 * WEI / ETHERIUM_PRICE) => 6,
                                    x if x < (5000 * WEI / ETHERIUM_PRICE) => 8,
                                    _ => 12,
                                },
                                // # Bitcoin
                                // < $100 / $6400 - 0 conf
                                // < $500 / $6400 - 1 conf
                                // < $1000 / $6400 - 2 conf
                                // > $1000 / $6400 - 3 conf
                                Currency::Btc => match total_value.raw() {
                                    x if x < (100 * SATOSHI / BITCOIN_PRICE) => 0,
                                    x if x < (500 * SATOSHI / BITCOIN_PRICE) => 1,
                                    x if x < (1000 * SATOSHI / BITCOIN_PRICE) => 2,
                                    _ => 3,
                                },
                            };

                            //checking for enough confirmations
                            if blockchain_transaction.confirmations < enough_confirms {
                                return Ok(());
                            }

                            //checking blockchain hash already seen
                            if seen_hashes_repo
                                .get(blockchain_transaction.hash.clone(), blockchain_transaction.currency)?
                                .is_some()
                            {
                                return Ok(());
                            }

                            // unifying from and to
                            let (from, to) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert))?;

                            let unified_hash = blockchain_transaction.hash.clone();
                            // withdraw
                            if let Some(transaction) = transactions_repo.get_by_blockchain_tx(unified_hash.clone())? {
                                // checking that `from` account exists in accounts but no `to` in accounts
                                let mut to_not_exists = true;
                                for (address, _) in to {
                                    to_not_exists &= accounts_repo
                                        .get_by_address(address.clone(), blockchain_transaction.currency, AccountKind::Cr)?
                                        .is_none()
                                }

                                if accounts_repo.get(transaction.dr_account_id)?.is_none() {
                                    let comment = format!("Withdraw transaction dr account {} does not exists.", transaction.dr_account_id);
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else if !to_not_exists {
                                    let comment = "Withdraw transaction contains our account in `to` field.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else if transaction.status == TransactionStatus::Done {
                                    let comment = "Withdraw transaction is already in done state.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else {
                                    transactions_repo.update_status(unified_hash.clone(), TransactionStatus::Done)?;
                                    pending_blockchain_transactions_repo.delete(unified_hash)?;
                                    blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                                }
                            } else {
                                // checking that `from` accounts not exist
                                let mut from_not_exists = true;
                                for address in from {
                                    from_not_exists &= accounts_repo
                                        .get_by_address(address.clone(), blockchain_transaction.currency, AccountKind::Cr)?
                                        .is_none()
                                }
                                if from_not_exists {
                                    // deposit
                                    for (blockchain_transaction_to, blockchain_transaction_value) in to {
                                        let mut added_transactions = false;
                                        if let Some(cr_account) = accounts_repo.get_by_address(
                                            blockchain_transaction_to.clone(),
                                            blockchain_transaction.currency,
                                            AccountKind::Cr,
                                        )? {
                                            if let Some(dr_account) = accounts_repo.get_by_address(
                                                blockchain_transaction_to.clone(),
                                                blockchain_transaction.currency,
                                                AccountKind::Dr,
                                            )? {
                                                let new_transaction = NewTransaction {
                                                    id: TransactionId::generate(),
                                                    user_id: cr_account.user_id,
                                                    dr_account_id: dr_account.id,
                                                    cr_account_id: cr_account.id,
                                                    currency: blockchain_transaction.currency,
                                                    value: blockchain_transaction_value,
                                                    status: TransactionStatus::Done,
                                                    blockchain_tx_id: Some(blockchain_transaction.hash.clone()),
                                                    hold_until: None,
                                                    fee: blockchain_transaction.fee,
                                                };
                                                transactions_repo.create(new_transaction)?;
                                                added_transactions = true;
                                            } else {
                                                return Err(
                                                    ectx!(err RepoErrorContex::AccountsPair, RepoErrorKind::Internal => blockchain_transaction_to.clone()),
                                                );
                                            }
                                        }
                                        if added_transactions {
                                            //adding blockchain transaction to db
                                            blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                                        }
                                    }
                                } else {
                                    let comment = "Withdraw transaction hash does not exists, but `from` field contains our account.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                }

                            }
                            //adding blockchain hash to already seen 
                            seen_hashes_repo.create(blockchain_transaction.clone().into())?;
                            Ok(())
                        }).map_err(ectx!(ErrorSource::Repo, ErrorKind::Internal))
                }),
        )
    }
}
