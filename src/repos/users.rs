//! Users repo, presents CRUD operations with db for users
use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::Connection;
use failure::Fail;

use super::types::RepoResult;
use models::UserId;
use models::{NewUser, UpdateUser, User};
use repos::ErrorKind;
use schema::users::dsl::*;

/// Users repository, responsible for handling users
pub struct UsersRepoImpl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> {
    pub db_conn: &'a T,
}

pub trait UsersRepo {
    fn get(&self, user_id: UserId) -> RepoResult<Option<User>>;
    fn create(&self, payload: NewUser) -> RepoResult<User>;
    fn update(&self, user_id: UserId, payload: UpdateUser) -> RepoResult<User>;
    fn delete(&self, user_id: UserId) -> RepoResult<User>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UsersRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T) -> Self {
        Self { db_conn }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UsersRepo for UsersRepoImpl<'a, T> {
    fn get(&self, user_id_arg: UserId) -> RepoResult<Option<User>> {
        let query = users.find(user_id_arg);
        query
            .get_result(self.db_conn)
            .optional()
            .map_err(ectx!(ErrorKind::Internal => user_id_arg))
    }

    fn create(&self, payload: NewUser) -> RepoResult<User> {
        diesel::insert_into(users)
            .values(payload.clone())
            .get_result::<User>(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => payload))
    }

    fn update(&self, user_id_arg: UserId, payload: UpdateUser) -> RepoResult<User> {
        let filter = users.filter(id.eq(user_id_arg));
        diesel::update(filter)
            .set(&payload)
            .get_result(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => user_id_arg, payload))
    }

    fn delete(&self, user_id_arg: UserId) -> RepoResult<User> {
        let filtered = users.find(user_id_arg);
        diesel::delete(filtered)
            .get_result(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => user_id_arg))
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;

    use std::env;

    use diesel::prelude::*;

    use super::*;
    use models::*;
    use config::Config;

    pub fn connection() -> PgConnection {
        let config = Config::new().unwrap();
        let conn = PgConnection::establish(&config.database.url).unwrap();
        conn.begin_test_transaction().unwrap();
        conn
    }

    #[test]
    fn crud() {
        let test_connection = connection();
        let repo = UsersRepoImpl::new(&test_connection);
        let new_user = NewUser::default();

        assert!(repo.create(new_user.clone()).is_ok());
        assert!(repo.get(new_user.id).is_ok());
        let payload = UpdateUser {
            name: Some("test".to_string()),
            authentication_token: None,
        };
        assert!(repo.update(new_user.id, payload).is_ok());
        assert!(repo.delete(new_user.id).is_ok());
    }
}
