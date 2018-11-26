use diesel;
use diesel::dsl::count;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::pending_blockchain_transactions::dsl::*;

pub trait PendingBlockchainTransactionsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewPendingBlockchainTransactionDB) -> RepoResult<PendingBlockchainTransactionDB>;
    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>>;
    fn count(&self) -> RepoResult<u64>;
    fn delete(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>>;
}

#[derive(Clone, Default)]
pub struct PendingBlockchainTransactionsRepoImpl;

impl PendingBlockchainTransactionsRepo for PendingBlockchainTransactionsRepoImpl {
    fn count(&self) -> RepoResult<u64> {
        with_tls_connection(|conn| {
            pending_blockchain_transactions
                .select(count(hash))
                .first(conn)
                .map(|x: i64| x as u64)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind)
                })
        })
    }

    fn create(&self, payload: NewPendingBlockchainTransactionDB) -> RepoResult<PendingBlockchainTransactionDB> {
        with_tls_connection(|conn| {
            diesel::insert_into(pending_blockchain_transactions)
                .values(payload.clone())
                .get_result::<PendingBlockchainTransactionDB>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }
    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>> {
        with_tls_connection(|conn| {
            pending_blockchain_transactions
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
    fn delete(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>> {
        with_tls_connection(|conn| {
            let filtered = pending_blockchain_transactions.filter(hash.eq(hash_.clone()));
            diesel::delete(filtered).get_result(conn).optional().map_err(move |e| {
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
    fn pending_blockchain_transactions_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let pending_blockchain_transactions_repo = PendingBlockchainTransactionsRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewPendingBlockchainTransactionDB::default();
            let res = pending_blockchain_transactions_repo.create(trans);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn pending_blockchain_transactions_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let pending_blockchain_transactions_repo = PendingBlockchainTransactionsRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewPendingBlockchainTransactionDB::default();
            let transaction = pending_blockchain_transactions_repo.create(trans)?;
            let res = pending_blockchain_transactions_repo.get(transaction.hash);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn pending_blockchain_transactions_delete() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let pending_blockchain_transactions_repo = PendingBlockchainTransactionsRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewPendingBlockchainTransactionDB::default();
            let transaction = pending_blockchain_transactions_repo.create(trans)?;
            let res = pending_blockchain_transactions_repo.delete(transaction.hash);
            assert!(res.is_ok());
            res
        }));
    }
}
