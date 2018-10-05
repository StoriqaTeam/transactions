mod error;
mod users;

pub use self::error::*;
pub use self::users::*;

use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;
use failure::Fail;
use futures::Future;
use futures_cpupool::CpuPool;
use r2d2::Pool;
use r2d2::{ManageConnection, PooledConnection};

use repos::ReposFactory;

pub type ServiceFuture<T> = Box<Future<Item = T, Error = Error> + Send>;

/// Service
pub struct Service<T, M, F>
where
    T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
    M: ManageConnection<Connection = T>,
    F: ReposFactory<T>,
{
    db_pool: Pool<M>,
    cpu_pool: CpuPool,
    repo_factory: F,
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Service<T, M, F>
{
    /// Create a new service
    pub fn new(db_pool: Pool<M>, cpu_pool: CpuPool, repo_factory: F) -> Self {
        Self {
            db_pool,
            cpu_pool,
            repo_factory,
        }
    }

    pub fn spawn_on_pool<R, Func>(&self, f: Func) -> ServiceFuture<R>
    where
        Func: FnOnce(PooledConnection<M>) -> Result<R, Error> + Send + 'static,
        R: Send + 'static,
    {
        let db_pool = self.db_pool.clone();
        let cpu_pool = self.cpu_pool.clone();
        Box::new(cpu_pool.spawn_fn(move || db_pool.get().map_err(ectx!(ErrorKind::Internal)).and_then(f)))
    }
}

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > Clone for Service<T, M, F>
{
    fn clone(&self) -> Self {
        Self {
            db_pool: self.db_pool.clone(),
            cpu_pool: self.cpu_pool.clone(),
            repo_factory: self.repo_factory.clone(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;
    extern crate futures;
    extern crate futures_cpupool;
    extern crate r2d2;

    use std::env;
    use std::error::Error;
    use std::fmt;
    use std::fs::File;
    use std::io::prelude::*;
    use std::sync::Arc;
    use std::time::SystemTime;

    use diesel::connection::AnsiTransactionManager;
    use diesel::connection::SimpleConnection;
    use diesel::deserialize::QueryableByName;
    use diesel::pg::Pg;
    use diesel::prelude::*;
    use diesel::query_builder::AsQuery;
    use diesel::query_builder::QueryFragment;
    use diesel::query_builder::QueryId;
    use diesel::sql_types::HasSqlType;
    use diesel::Connection;
    use diesel::ConnectionResult;
    use diesel::QueryResult;
    use diesel::Queryable;
    use futures_cpupool::CpuPool;
    use r2d2::ManageConnection;

    use config::Config;
    use models::*;
    use repos::repo_factory::tests::ReposFactoryMock;
    use repos::repo_factory::ReposFactory;
    use repos::types::RepoResult;
    use repos::users::UsersRepo;
    use services::Service;

    pub const MOCK_REPO_FACTORY: ReposFactoryMock = ReposFactoryMock {};

    pub fn create_service() -> Service<MockConnection, MockConnectionManager, ReposFactoryMock> {
        let manager = MockConnectionManager::default();
        let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");
        let cpu_pool = CpuPool::new(1);
        Service::new(db_pool, cpu_pool, MOCK_REPO_FACTORY)
    }

    #[derive(Default)]
    pub struct MockConnection {
        tr: AnsiTransactionManager,
    }

    impl Connection for MockConnection {
        type Backend = Pg;
        type TransactionManager = AnsiTransactionManager;

        fn establish(_database_url: &str) -> ConnectionResult<MockConnection> {
            Ok(MockConnection::default())
        }

        fn execute(&self, _query: &str) -> QueryResult<usize> {
            unimplemented!()
        }

        fn query_by_index<T, U>(&self, _source: T) -> QueryResult<Vec<U>>
        where
            T: AsQuery,
            T::Query: QueryFragment<Pg> + QueryId,
            Pg: HasSqlType<T::SqlType>,
            U: Queryable<T::SqlType, Pg>,
        {
            unimplemented!()
        }

        fn query_by_name<T, U>(&self, _source: &T) -> QueryResult<Vec<U>>
        where
            T: QueryFragment<Pg> + QueryId,
            U: QueryableByName<Pg>,
        {
            unimplemented!()
        }

        fn execute_returning_count<T>(&self, _source: &T) -> QueryResult<usize>
        where
            T: QueryFragment<Pg> + QueryId,
        {
            unimplemented!()
        }

        fn transaction_manager(&self) -> &Self::TransactionManager {
            &self.tr
        }
    }

    impl SimpleConnection for MockConnection {
        fn batch_execute(&self, _query: &str) -> QueryResult<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    pub struct MockConnectionManager;

    impl ManageConnection for MockConnectionManager {
        type Connection = MockConnection;
        type Error = MockError;

        fn connect(&self) -> Result<MockConnection, MockError> {
            Ok(MockConnection::default())
        }

        fn is_valid(&self, _conn: &mut MockConnection) -> Result<(), MockError> {
            Ok(())
        }

        fn has_broken(&self, _conn: &mut MockConnection) -> bool {
            false
        }
    }

    #[derive(Debug)]
    pub struct MockError {}

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "SuperError is here!")
        }
    }

    impl Error for MockError {
        fn description(&self) -> &str {
            "I'm the superhero of errors"
        }

        fn cause(&self) -> Option<&Error> {
            None
        }
    }
}
