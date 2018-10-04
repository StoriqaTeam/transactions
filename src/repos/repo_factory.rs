use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::Connection;

use repos::*;

pub trait ReposFactory<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static>:
    Clone + Send + Sync + 'static
{
    fn create_users_repo<'a>(&self, db_conn: &'a C) -> Box<UsersRepo + 'a>;
}

#[derive(Default, Copy, Clone, Debug)]
pub struct ReposFactoryImpl;

impl<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ReposFactory<C> for ReposFactoryImpl {
    fn create_users_repo<'a>(&self, db_conn: &'a C) -> Box<UsersRepo + 'a> {
        Box::new(UsersRepoImpl::new(db_conn)) as Box<UsersRepo>
    }
}

#[cfg(test)]
pub mod tests {
    extern crate base64;
    extern crate diesel;
    extern crate futures;
    extern crate futures_cpupool;
    extern crate hyper;
    extern crate r2d2;
    extern crate rand;
    extern crate serde_json;
    extern crate sha3;
    extern crate stq_http;
    extern crate tokio_core;

    use std::error::Error;
    use std::fmt;
    use std::fs::File;
    use std::io::prelude::*;
    use std::sync::Arc;
    use std::time::SystemTime;

    use base64::encode;
    use diesel::connection::AnsiTransactionManager;
    use diesel::connection::SimpleConnection;
    use diesel::deserialize::QueryableByName;
    use diesel::pg::Pg;
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
    use sha3::{Digest, Sha3_256};
    use tokio_core::reactor::Handle;

    use stq_static_resources::{Provider, TokenType};
    use stq_types::{RoleId, UserId, UsersRole};

    use config::Config;
    use controller::context::{DynamicContext, StaticContext};
    use models::*;
    use repos::identities::IdentitiesRepo;
    use repos::repo_factory::ReposFactory;
    use repos::types::RepoResult;
    use repos::users::UsersRepo;
    use services::Service;

    pub const MOCK_REPO_FACTORY: ReposFactoryMock = ReposFactoryMock {};
    pub const MOCK_USERS: UsersRepoMock = UsersRepoMock {};

    #[derive(Default, Copy, Clone)]
    pub struct ReposFactoryMock;

    impl<C: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> ReposFactory<C> for ReposFactoryMock {
        fn create_users_repo<'a>(&self, _db_conn: &'a C) -> Box<UsersRepo + 'a> {
            Box::new(UsersRepoMock::default()) as Box<UsersRepo>
        }
    }

    #[derive(Clone, Default)]
    pub struct UsersRepoMock;

    pub fn create_user(id: UserId, name: String) -> User {
        User {
            id,
            name,
            ..Default::default()
        }
    }

    impl UsersRepo for UsersRepoMock {
        fn read(&self, user_id: UserId) -> RepoResult<Option<User>> {
            let user = create_user(user_id, MOCK_EMAIL.to_string());
            Ok(Some(user))
        }

        fn create(&self, payload: NewUser) -> RepoResult<User> {
            let user = create_user(UserId(1), payload.email);
            Ok(user)
        }

        fn delete(&self, user_id: UserId) -> RepoResult<User> {
            let mut user = create_user(user_id, MOCK_EMAIL.to_string());
            user.is_active = false;
            Ok(user)
        }
    }

    pub fn create_service(
        user_id: Option<UserId>,
        handle: Arc<Handle>,
    ) -> Service<MockConnection, MockConnectionManager, ReposFactoryMock> {
        let manager = MockConnectionManager::default();
        let db_pool = r2d2::Pool::builder().build(manager).expect("Failed to create connection pool");
        let cpu_pool = CpuPool::new(1);

        let config = Config::new().unwrap();
        let client = stq_http::client::Client::new(&config.to_http_config(), &handle);
        let client_handle = client.handle();
        let static_context = StaticContext::new(
            db_pool,
            cpu_pool,
            client_handle,
            Arc::new(config),
            MOCK_REPO_FACTORY,
            jwt_private_key,
        );
        let dynamic_context = DynamicContext::new(user_id);

        Service::new(static_context, dynamic_context)
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
