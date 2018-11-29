use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::blockchain_transactions::dsl::*;

pub trait BlockchainTransactionsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewBlockchainTransactionDB) -> RepoResult<BlockchainTransactionDB>;
    fn upsert(&self, payload: NewBlockchainTransactionDB) -> RepoResult<BlockchainTransactionDB>;
    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<BlockchainTransactionDB>>;
}

#[derive(Clone, Default)]
pub struct BlockchainTransactionsRepoImpl;

impl BlockchainTransactionsRepo for BlockchainTransactionsRepoImpl {
    fn create(&self, payload: NewBlockchainTransactionDB) -> RepoResult<BlockchainTransactionDB> {
        with_tls_connection(|conn| {
            diesel::insert_into(blockchain_transactions)
                .values(payload.clone())
                .get_result::<BlockchainTransactionDB>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }
    fn upsert(&self, payload: NewBlockchainTransactionDB) -> RepoResult<BlockchainTransactionDB> {
        with_tls_connection(|conn| {
            diesel::insert_into(blockchain_transactions)
                .values(payload.clone())
                .on_conflict((hash, currency))
                .do_nothing()
                .get_result::<BlockchainTransactionDB>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }

    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<BlockchainTransactionDB>> {
        with_tls_connection(|conn| {
            blockchain_transactions
                .filter(hash.eq(hash_.clone()))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => hash_)
                })
        })
    }
}

#[cfg(test)]
pub mod tests {
    use diesel::r2d2::ConnectionManager;
    use diesel::PgConnection;
    use futures_cpupool::CpuPool;
    use r2d2;
    use tokio_core::reactor::Core;

    use super::*;
    use config::Config;
    use repos::DbExecutorImpl;

    fn create_executor() -> DbExecutorImpl {
        let config = Config::new().unwrap();
        let manager = ConnectionManager::<PgConnection>::new(config.database.url);
        let db_pool = r2d2::Pool::builder().build(manager).unwrap();
        let cpu_pool = CpuPool::new(1);
        DbExecutorImpl::new(db_pool.clone(), cpu_pool.clone())
    }

    #[test]
    fn blockchain_transactions_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let blockchain_transactions_repo = BlockchainTransactionsRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewBlockchainTransactionDB::default();
            let res = blockchain_transactions_repo.create(trans);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn blockchain_transactions_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let blockchain_transactions_repo = BlockchainTransactionsRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewBlockchainTransactionDB::default();
            let transaction = blockchain_transactions_repo.create(trans)?;
            let res = blockchain_transactions_repo.get(transaction.hash);
            assert!(res.is_ok());
            res
        }));
    }
}
