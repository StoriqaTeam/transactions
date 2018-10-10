use std::sync::Arc;

use futures::IntoFuture;
use validator::Validate;

use super::auth::AuthService;
use super::error::*;
use models::*;
use prelude::*;
use repos::{DbExecutor, UsersRepo};

#[derive(Clone)]
pub struct UsersServiceImpl<E: DbExecutor> {
    auth_service: Arc<AuthService>,
    users_repo: Arc<UsersRepo>,
    db_executor: E,
}

impl<E: DbExecutor> UsersServiceImpl<E> {
    pub fn new(auth_service: Arc<AuthService>, users_repo: Arc<UsersRepo>, db_executor: E) -> Self {
        Self {
            auth_service,
            users_repo,
            db_executor,
        }
    }
}

pub trait UsersService: Send + Sync + 'static {
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send>;
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> Box<Future<Item = Option<User>, Error = Error> + Send>;
    // fn get_user(&self, user_id: UserId) -> Box<Future<Item = Option<User>, Error = Error> + Send>;
    // fn update_user(&self, user_id: UserId, payload: UpdateUser) -> Box<Future<Item = User, Error = Error> + Send>;
    // fn delete_user(&self, user_id: UserId) -> Box<Future<Item = User, Error = Error> + Send>;
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
    // fn get_user(&self, user_id: UserId) -> Box<Future<Item = Option<User>, Error = Error> + Send> {
    //     let users_repo = self.users_repo.clone();
    //     self.db_executor
    //         .execute(move || users_repo.get(user_id).map_err(ectx!(ErrorKind::Internal => user_id)))
    // }
    // fn update_user(&self, user_id: UserId, payload: UpdateUser) -> Box<Future<Item = User, Error = Error> + Send> {
    //     let users_repo = self.users_repo.clone();
    //     let db_executor = self.db_executor.clone();
    //     Box::new(
    //         payload
    //             .validate()
    //             .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => payload))
    //             .into_future()
    //             .and_then(move |_| {
    //                 db_executor.execute(move || {
    //                     users_repo
    //                         .update(user_id, payload.clone())
    //                         .map_err(ectx!(ErrorKind::Internal => user_id, payload))
    //                 })
    //             }),
    //     )
    // }
    // fn delete_user(&self, user_id: UserId) -> Box<Future<Item = User, Error = Error> + Send> {
    //     let users_repo = self.users_repo.clone();
    //     self.db_executor
    //         .execute(move || users_repo.delete(user_id).map_err(ectx!(ErrorKind::Internal => user_id)))
    // }

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
    use services::*;
    use tokio_core::reactor::Core;

    #[test]
    fn test_create() {
        let new_user = NewUser::default();
        let token = new_user.authentication_token.clone();
        let auth_service = Arc::new(AuthServiceMock::new(vec![(token.clone(), new_user.id)]));
        let users_repo = Arc::new(UsersRepoMock::default());
        let db_executor = DbExecutorMock::default();
        let users_service = UsersServiceImpl::new(auth_service, users_repo, db_executor);
        let mut core = Core::new().unwrap();
        let new_user = NewUser::default();

        // creates user
        let user = core.run(users_service.create_user(new_user));
        assert!(user.is_ok());
    }

    #[test]
    fn test_get_by_auth_token() {
        let new_user = NewUser::default();
        let token = new_user.authentication_token.clone();
        let auth_service = Arc::new(AuthServiceMock::new(vec![(token.clone(), new_user.id)]));
        let users_repo = Arc::new(UsersRepoMock::default());
        let db_executor = DbExecutorMock::default();
        let users_service = UsersServiceImpl::new(auth_service, users_repo, db_executor);
        let mut core = Core::new().unwrap();
        let new_user = NewUser::default();

        // creates user
        let user = core.run(users_service.create_user(new_user));
        assert!(user.is_ok());

        // creates user
        let user = core
            .run(users_service.find_user_by_authentication_token(user.unwrap().authentication_token))
            .unwrap();
        assert!(user.is_some());
    }

}
