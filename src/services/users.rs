use std::sync::Arc;

use futures::IntoFuture;
use validator::Validate;

use super::error::*;
use models::*;
use prelude::*;
use repos::{DbExecutor, UsersRepo};

#[derive(Clone)]
pub struct UsersServiceImpl<E: DbExecutor> {
    users_repo: Arc<UsersRepo>,
    db_executor: E,
}

impl<E: DbExecutor> UsersServiceImpl<E> {
    pub fn new(users_repo: Arc<UsersRepo>, db_executor: E) -> Self {
        Self { users_repo, db_executor }
    }
}

pub trait UsersService: Send + Sync + 'static {
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send>;
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> Box<Future<Item = Option<User>, Error = Error> + Send>;
}

impl<E: DbExecutor> UsersService for UsersServiceImpl<E> {
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send> {
        let users_repo = self.users_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute(move || users_repo.create(input.clone()).map_err(ectx!(ErrorKind::Internal => input)))
                }),
        )
    }
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> Box<Future<Item = Option<User>, Error = Error> + Send> {
        let users_repo = self.users_repo.clone();
        self.db_executor.execute(move || {
            users_repo
                .find_user_by_authentication_token(token.clone())
                .map_err(ectx!(ErrorKind::Internal => token))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repos::*;
    use tokio_core::reactor::Core;

    fn create_users_service() -> UsersServiceImpl<DbExecutorMock> {
        let users_repo = Arc::new(UsersRepoMock::default());
        let db_executor = DbExecutorMock::default();
        UsersServiceImpl::new(users_repo, db_executor)
    }

    #[test]
    fn test_create() {
        let mut core = Core::new().unwrap();
        let users_service = create_users_service();
        let mut new_user = NewUser::default();
        new_user.name = "fksjdlfkjsdlkfdlksf".to_string();
        new_user.authentication_token = AuthenticationToken::new("fksjdlfkjsdlkfdlksf".to_string());
        let user = core.run(users_service.create_user(new_user));
        assert!(user.is_ok());
    }

    #[test]
    fn test_get_by_auth_token() {
        let mut core = Core::new().unwrap();
        let users_service = create_users_service();
        let mut new_user = NewUser::default();
        new_user.name = "fksjdlfkjsdlkfdlksf".to_string();
        new_user.authentication_token = AuthenticationToken::new("fksjdlfkjsdlkfdlksf".to_string());
        let user = core.run(users_service.create_user(new_user));
        assert!(user.is_ok());
        let user = core
            .run(users_service.find_user_by_authentication_token(user.unwrap().authentication_token))
            .unwrap();
        assert!(user.is_some());
    }

}
