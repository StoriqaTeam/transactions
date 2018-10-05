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
    extern crate serde_json;

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
    use repos::repo_factory::ReposFactory;
    use repos::types::RepoResult;
    use repos::users::UsersRepo;

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

    pub fn create_user(id: UserId) -> User {
        User { id, ..Default::default() }
    }

    impl UsersRepo for UsersRepoMock {
        fn get(&self, user_id: UserId) -> RepoResult<Option<User>> {
            let user = create_user(user_id);
            Ok(Some(user))
        }

        fn create(&self, payload: NewUser) -> RepoResult<User> {
            let user = create_user(payload.id);
            Ok(user)
        }

        fn delete(&self, user_id: UserId) -> RepoResult<User> {
            let mut user = create_user(user_id);
            Ok(user)
        }

        fn update(&self, user_id: UserId, payload: UpdateUser) -> RepoResult<User> {
            let mut user = create_user(user_id);
            if let Some(name) = payload.name {
                user.name = name;
            }
            if let Some(authentication_token) = payload.authentication_token {
                user.authentication_token = authentication_token;
            }
            Ok(user)
        }
    }
}
