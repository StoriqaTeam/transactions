use std::sync::Arc;

use super::error::*;
use super::ServiceFuture;
use futures::future;
use models::*;
use prelude::*;
use repos::{DbExecutor, UsersRepo};

pub trait AuthService: Send + Sync + 'static {
    fn authenticate(&self, maybe_token: Option<AuthenticationToken>) -> ServiceFuture<User>;
}

#[derive(Clone)]
pub struct AuthServiceImpl<E: DbExecutor> {
    users_repo: Arc<UsersRepo>,
    db_executor: E,
}

impl<E: DbExecutor> AuthServiceImpl<E> {
    pub fn new(users_repo: Arc<UsersRepo>, db_executor: E) -> Self {
        AuthServiceImpl { users_repo, db_executor }
    }
}

impl<E: DbExecutor> AuthService for AuthServiceImpl<E> {
    fn authenticate(&self, maybe_token: Option<AuthenticationToken>) -> ServiceFuture<User> {
        let token = match maybe_token {
            Some(t) => t,
            None => return Box::new(future::err(ErrorKind::Unauthorized.into())),
        };
        let users_repo = self.users_repo.clone();
        let token_clone = token.clone();
        let token_clone2 = token.clone();
        Box::new(self.db_executor.execute(move || {
            users_repo
                .find_user_by_authentication_token(token)
                .map_err(ectx!(convert => token_clone))
                .and_then(move |maybe_user| maybe_user.ok_or(ectx!(err ErrorContext::NoAuthToken, ErrorKind::Unauthorized => token_clone2)))
        }))
    }
}
