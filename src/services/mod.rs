mod error;
mod users;

pub use self::error::*;
pub use self::users::*;

use std::sync::Arc;

use diesel::pg::PgConnection;
use diesel::r2d2::ConnectionManager;
use failure::Fail;
use futures::Future;
use futures_cpupool::CpuPool;
use r2d2::PooledConnection;

use prelude::*;
use repos::ReposFactory;

pub type ServiceFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

/// Service
#[derive(Clone)]
pub struct Service {
    db_pool: PgConnectionPool,
    cpu_pool: CpuPool,
    repo_factory: Arc<ReposFactory>,
}

impl Service {
    /// Create a new service
    pub fn new(db_pool: PgConnectionPool, cpu_pool: CpuPool, repo_factory: Arc<ReposFactory>) -> Self {
        Self {
            db_pool,
            cpu_pool,
            repo_factory,
        }
    }

    pub fn spawn_on_pool<R, Func>(&self, f: Func) -> ServiceFuture<R>
    where
        Func: FnOnce(PooledConnection<ConnectionManager<PgConnection>>) -> Result<R, Error> + Send + 'static,
        R: Send + 'static,
    {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorKind::Internal)).and_then(f)))
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;
    extern crate futures;
    extern crate futures_cpupool;
    extern crate r2d2;

    use std::sync::Arc;

    use diesel::prelude::*;
    use diesel::r2d2::ConnectionManager;
    use futures_cpupool::CpuPool;

    use config::Config;
    use repos::repo_factory::tests::ReposFactoryMock;
    use services::Service;

    pub const MOCK_REPO_FACTORY: ReposFactoryMock = ReposFactoryMock {};

    pub fn create_service() -> Service {
        let config = Config::new().unwrap();
        let manager = ConnectionManager::<PgConnection>::new(config.database.url.clone());
        let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");
        let cpu_pool = CpuPool::new(1);
        Service::new(db_pool, cpu_pool, Arc::new(MOCK_REPO_FACTORY))
    }
}
