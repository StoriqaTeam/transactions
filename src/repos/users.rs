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
    /// Find specific user by ID
    fn get(&self, user_id: UserId) -> RepoResult<Option<User>>;

    /// Creates new user
    fn create(&self, payload: NewUser) -> RepoResult<User>;

    /// Updates specific user
    fn update(&self, user_id: UserId, payload: UpdateUser) -> RepoResult<User>;

    /// Deactivates specific user
    fn delete(&self, user_id: UserId) -> RepoResult<User>;
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UsersRepoImpl<'a, T> {
    pub fn new(db_conn: &'a T) -> Self {
        Self { db_conn }
    }
}

impl<'a, T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static> UsersRepo for UsersRepoImpl<'a, T> {
    /// Find specific user by ID
    fn get(&self, user_id_arg: UserId) -> RepoResult<Option<User>> {
        let query = users.find(user_id_arg);
        query
            .get_result(self.db_conn)
            .optional()
            .map_err(ectx!(ErrorKind::Internal => user_id_arg))
    }

    /// Creates new user
    fn create(&self, payload: NewUser) -> RepoResult<User> {
        diesel::insert_into(users)
            .values(payload.clone())
            .get_result::<User>(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => payload))
    }

    /// Updates specific user
    fn update(&self, user_id_arg: UserId, payload: UpdateUser) -> RepoResult<User> {
        let filter = users.filter(id.eq(user_id_arg));
        diesel::update(filter)
            .set(&payload)
            .get_result(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => user_id_arg, payload))
    }

    /// Deactivates specific user
    fn delete(&self, user_id_arg: UserId) -> RepoResult<User> {
        let filtered = users.find(user_id_arg);
        diesel::delete(filtered)
            .get_result(self.db_conn)
            .map_err(ectx!(ErrorKind::Internal => user_id_arg))
    }
}
