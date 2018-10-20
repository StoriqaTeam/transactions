use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::sync::Arc;

use models::*;
use prelude::*;
use repos::error::Error as RepoError;
use repos::*;

pub const ETHERIUM_PRICE : u128 = 200; // 200$, price of 1 eth in gwei 
pub const BLOCKCHAIN_PRICE : u128 = 6400; // 6400$ price in satoshi

#[derive(Clone)]
pub struct BlockchainWorkerImpl<E: DbExecutor> {
    transactions_repo: Arc<TransactionsRepo>,
    acounts_repo: Arc<AcountsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainWorkerImpl<E> {
    pub fn new(
        transactions_repo: Arc<TransactionsRepo>,
        acounts_repo: Arc<AcountsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        db_executor: E,
    ) -> Self {
        BlockchainWorkerImpl {
            transactions_repo,
            acounts_repo,
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
                            x if x < (20 / ETHERIUM_PRICE * 1000000000) => 0,
                            x if x < (50 / ETHERIUM_PRICE * 1000000000) => 1,
                            x if x < (200 / ETHERIUM_PRICE * 1000000000) => 2,
                            x if x < (500 / ETHERIUM_PRICE * 1000000000) => 3,
                            x if x < (1000 / ETHERIUM_PRICE * 1000000000) => 4,
                            x if x < (2000 / ETHERIUM_PRICE * 1000000000) => 5,
                            x if x < (3000 / ETHERIUM_PRICE * 1000000000) => 6,
                            x if x < (5000 / ETHERIUM_PRICE * 1000000000) => 8,
                            _ => 12,
                        },
                        // # Bitcoin
                        // < $100 / $6400 - 0 conf
                        // < $500 / $6400 - 1 conf
                        // < $1000 / $6400 - 2 conf
                        // > $1000 / $6400 - 3 conf
                        Currency::Btc => match blockchain_transaction.value.raw() {
                            x if x < (100 / BLOCKCHAIN_PRICE * 1000000000) => 0,
                            x if x < (500 / BLOCKCHAIN_PRICE * 1000000000) => 1,
                            x if x < (1000 / BLOCKCHAIN_PRICE * 1000000000) => 2,
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

                    // withdraw
                    if transactions_repo.get_by_blockchain_tx(blockchain_transaction.hash.clone())?
                        .is_some()
                    {
                        transactions_repo.update_status(blockchain_transaction.hash.clone(), TransactionStatus::Done)?;    
                    }

                    // deposit
                    if let Some(cr_account) = accounts_repo.get_by_address(blockchain_transaction.to, AccountKind::Cr)?
                    {
                        if let Some(dr_account) = accounts_repo.get_by_address(blockchain_transaction.to, AccountKind::Dr)? {
                            let new_transaction =  NewTransaction {
                                id: TransactionsId::generate(),
                                user_id: ,
                                dr_account_id: ,
                                cr_account_id: ,
                                currency: ,
                                value: ,
                                status: ,
                                blockchain_tx_id: ,
                                hold_until: ,
                            };
                            transactions_repo.create(new_transaction)?;    
                        }
                    }
                    
                    blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                    seen_hashes_repo.create(blockchain_transaction.clone().into())?;
                    Ok(())
                }).map_err(|_| IoErrorKind::Other.into()),
        )
    }
}
