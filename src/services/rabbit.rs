use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::sync::Arc;

use super::error::*;
use models::*;
use prelude::*;
use repos::error::Error as RepoError;
use repos::{AccountsRepo, BlockchainTransactionsRepo, DbExecutor, SeenHashesRepo, TransactionsRepo};
use serde_json;

pub const ETHERIUM_PRICE: u128 = 200; // 200$, price of 1 eth in gwei
pub const BLOCKCHAIN_PRICE: u128 = 6400; // 6400$ price in satoshi

#[derive(Clone)]
pub struct BlockchainFetcher<E: DbExecutor> {
    transactions_repo: Arc<TransactionsRepo>,
    accounts_repo: Arc<AccountsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn new(
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<AccountsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        db_executor: E,
    ) -> Self {
        BlockchainFetcher {
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
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
        let db_executor = self.db_executor.clone();
        Box::new(
            String::from_utf8(data.clone())
                .map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => data_clone))
                .into_future()
                .and_then(|s| serde_json::from_str(&s).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => s)))
                .and_then(move |blockchain_transaction: BlockchainTransaction| {
                    db_executor
                        .execute_transaction(move || -> Result<(), RepoError> {
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
                                Currency::Eth | Currency::Stq => match blockchain_transaction.value.raw() {
                                    x if x < (20 * 1000000000 / ETHERIUM_PRICE) => 0,
                                    x if x < (50 * 1000000000 / ETHERIUM_PRICE) => 1,
                                    x if x < (200 * 1000000000 / ETHERIUM_PRICE) => 2,
                                    x if x < (500 * 1000000000 / ETHERIUM_PRICE) => 3,
                                    x if x < (1000 * 1000000000 / ETHERIUM_PRICE) => 4,
                                    x if x < (2000 * 1000000000 / ETHERIUM_PRICE) => 5,
                                    x if x < (3000 * 1000000000 / ETHERIUM_PRICE) => 6,
                                    x if x < (5000 * 1000000000 / ETHERIUM_PRICE) => 8,
                                    _ => 12,
                                },
                                // # Bitcoin
                                // < $100 / $6400 - 0 conf
                                // < $500 / $6400 - 1 conf
                                // < $1000 / $6400 - 2 conf
                                // > $1000 / $6400 - 3 conf
                                Currency::Btc => match blockchain_transaction.value.raw() {
                                    x if x < (100 * 1000000000 / BLOCKCHAIN_PRICE) => 0,
                                    x if x < (500 * 1000000000 / BLOCKCHAIN_PRICE) => 1,
                                    x if x < (1000 * 1000000000 / BLOCKCHAIN_PRICE) => 2,
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

                            // withdraw
                            if transactions_repo
                                .get_by_blockchain_tx(blockchain_transaction.hash.clone())?
                                .is_some()
                            {
                                transactions_repo.update_status(blockchain_transaction.hash.clone(), TransactionStatus::Done)?;
                            }

                            // deposit
                            if let Some(cr_account) = accounts_repo.get_by_address(blockchain_transaction.to.clone(), AccountKind::Cr)? {
                                if let Some(dr_account) =
                                    accounts_repo.get_by_address(blockchain_transaction.to.clone(), AccountKind::Dr)?
                                {
                                    let new_transaction = NewTransaction {
                                        id: TransactionId::generate(),
                                        user_id: cr_account.user_id,
                                        dr_account_id: dr_account.id,
                                        cr_account_id: cr_account.id,
                                        currency: blockchain_transaction.currency,
                                        value: blockchain_transaction.value,
                                        status: TransactionStatus::Done,
                                        blockchain_tx_id: Some(blockchain_transaction.hash.clone()),
                                        hold_until: None,
                                    };
                                    transactions_repo.create(new_transaction)?;
                                }
                            }

                            //adding blockchain transaction to db
                            blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;

                            //adding blockchain hash to already seen
                            seen_hashes_repo.create(blockchain_transaction.clone().into())?;

                            Ok(())
                        }).map_err(ectx!(ErrorSource::Repo, ErrorKind::Internal))
                }),
        )
    }
}
