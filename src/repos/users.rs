//! Users repo, presents CRUD operations with db for users
use diesel;
use diesel::connection::AnsiTransactionManager;
use diesel::dsl::exists;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_dsl::RunQueryDsl;
use diesel::select;
use diesel::Connection;
use failure::Error as FailureError;
use failure::Fail;

use models::UserId;
use super::types::RepoResult;
use models::{NewUser, UpdateUser, User, UsersSearchTerms};
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
        let query = users.find(user_id_arg.clone());

        query
            .get_result(self.db_conn)
            .optional()
            .map_err(From::from)
            .and_then(|user: Option<User>| {
                Ok(user)
            }).map_err(|e: FailureError| e.context(format!("Find specific user {} error occured", user_id_arg)).into())
    }

    /// Creates new user
    fn create(&self, payload: NewUser) -> RepoResult<User> {
        let query_user = diesel::insert_into(users).values(&payload);
        acl::check(&*self.acl, Resource::Users, Action::Create, self, None)?;
        query_user
            .get_result::<User>(self.db_conn)
            .map_err(|e| e.context(format!("Create a new user {:?} error occured", payload)).into())
    }

    /// Deactivates specific user
    fn delete(&self, user_id_arg: UserId) -> RepoResult<User> {
        let query = users.find(user_id_arg.clone());

        query
            .get_result(self.db_conn)
            .map_err(From::from)
            .and_then(|user: User| acl::check(&*self.acl, Resource::Users, Action::Delete, self, Some(&user)))
            .and_then(|_| {
                let filter = users.filter(id.eq(user_id_arg.clone())).filter(is_active.eq(true));
                let query = diesel::update(filter).set(is_active.eq(false));

                query.get_result(self.db_conn).map_err(From::from)
            }).map_err(|e: FailureError| e.context(format!("Deactivates user {:?} error occured", user_id_arg)).into())
    }

}
