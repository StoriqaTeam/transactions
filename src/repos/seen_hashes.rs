use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::seen_hashes::dsl::*;

pub trait SeenHashesRepo: Send + Sync + 'static {
    fn create(&self, payload: NewSeenHashes) -> RepoResult<SeenHashes>;
    fn upsert(&self, payload: NewSeenHashes) -> RepoResult<SeenHashes>;
    fn get(&self, hash_: BlockchainTransactionId, currency_: Currency) -> RepoResult<Option<SeenHashes>>;
}

#[derive(Clone, Default)]
pub struct SeenHashesRepoImpl;

impl SeenHashesRepo for SeenHashesRepoImpl {
    fn create(&self, payload: NewSeenHashes) -> RepoResult<SeenHashes> {
        with_tls_connection(|conn| {
            diesel::insert_into(seen_hashes)
                .values(payload.clone())
                .get_result::<SeenHashes>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }
    fn upsert(&self, payload: NewSeenHashes) -> RepoResult<SeenHashes> {
        with_tls_connection(|conn| {
            diesel::insert_into(seen_hashes)
                .values(payload.clone())
                .on_conflict(hash)
                .do_nothing()
                .get_result::<SeenHashes>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }

    fn get(&self, hash_: BlockchainTransactionId, currency_: Currency) -> RepoResult<Option<SeenHashes>> {
        with_tls_connection(|conn| {
            seen_hashes
                .filter(hash.eq(hash_.clone()))
                .filter(currency.eq(currency_))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => hash_, currency_)
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
    fn seen_hashes_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let seen_hashes_repo = SeenHashesRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewSeenHashes::default();
            let res = seen_hashes_repo.create(trans);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn seen_hashes_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let seen_hashes_repo = SeenHashesRepoImpl::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let trans = NewSeenHashes::default();
            let seen_hashes_ = seen_hashes_repo.create(trans)?;
            let res = seen_hashes_repo.get(seen_hashes_.hash, seen_hashes_.currency);
            assert!(res.is_ok());
            res
        }));
    }
}
