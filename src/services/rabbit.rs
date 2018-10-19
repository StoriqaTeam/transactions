use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::sync::Arc;

use models::*;
use prelude::*;
use repos::error::Error as RepoError;
use repos::*;

#[derive(Clone)]
pub struct BlockchainWorkerImpl<E: DbExecutor> {
    transactions_repo: Arc<TransactionsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainWorkerImpl<E> {
    pub fn new(
        transactions_repo: Arc<TransactionsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        db_executor: E,
    ) -> Self {
        BlockchainWorkerImpl {
            transactions_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
            db_executor,
        }
    }
}

impl<E: DbExecutor> BlockchainWorkerImpl<E> {
    pub fn work(&self, blockchain_transaction: BlockchainTransaction) -> impl Future<Item = (), Error = IoError> {
        let transactions_repo = self.transactions_repo.clone();
        let seen_hashes_repo = self.seen_hashes_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        Box::new(
            self.db_executor
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
                            x if x < (20 / 200) => 0,
                            x if x < (50 / 200) => 1,
                            x if x < (200 / 200) => 2,
                            x if x < (500 / 200) => 3,
                            x if x < (1000 / 200) => 4,
                            x if x < (2000 / 200) => 5,
                            x if x < (3000 / 200) => 6,
                            x if x < (5000 / 200) => 8,
                            _ => 12,
                        },
                        // # Bitcoin
                        // < $100 / $6400 - 0 conf
                        // < $500 / $6400 - 1 conf
                        // < $1000 / $6400 - 2 conf
                        // > $1000 / $6400 - 3 conf
                        Currency::Btc => match blockchain_transaction.value.raw() {
                            x if x < (100 / 6400) => 0,
                            x if x < (500 / 6400) => 1,
                            x if x < (1000 / 6400) => 2,
                            _ => 3,
                        },
                    };

                    if blockchain_transaction.confirmations < enough_confirms {
                        return Ok(());
                    }

                    if seen_hashes_repo
                        .get(blockchain_transaction.hash.clone(), blockchain_transaction.currency)?
                        .is_some()
                    {
                        return Ok(());
                    }

                    transactions_repo.update_status(blockchain_transaction.hash.clone(), TransactionStatus::Done)?;
                    blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                    seen_hashes_repo.create(blockchain_transaction.clone().into())?;
                    Ok(())
                }).map_err(|_| IoErrorKind::Other.into()),
        )
    }
}
