use std::sync::Arc;

use futures::IntoFuture;
use validator::Validate;

use super::auth::AuthService;
use super::error::*;
use models::*;
use prelude::*;
use repos::{DbExecutor, AccountsRepo, UsersRepo};

#[derive(Clone)]
pub struct AccountsServiceImpl<E: DbExecutor> {
    auth_service: Arc<AuthService>,
    accounts_repo: Arc<AccountsRepo>,
    users_repo: Arc<UsersRepo>,
    db_executor: E,
}

impl<E: DbExecutor> AccountsServiceImpl<E> {
    pub fn new(auth_service: Arc<AuthService>, accounts_repo: Arc<AccountsRepo>,users_repo: Arc<UsersRepo>, db_executor: E) -> Self {
        Self {
            auth_service,
            accounts_repo,
            users_repo,
            db_executor,
        }
    }
}

pub trait AccountsService: Send + Sync + 'static {
    fn create_account(&self, maybe_token: Option<AuthenticationToken>, input: NewAccount) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn get_account(&self, maybe_token: Option<AuthenticationToken>, account_id: AccountId) -> Box<Future<Item = Option<Account>, Error = Error> + Send>;
    fn update_account(&self, maybe_token: Option<AuthenticationToken>,  account_id: AccountId, payload: UpdateAccount) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn delete_account(&self, maybe_token: Option<AuthenticationToken>, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send>;
}

impl<E: DbExecutor> AccountsService for AccountsServiceImpl<E> {
    fn create_account(&self, maybe_token: Option<AuthenticationToken>,  input: NewAccount) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute(move || accounts_repo.create(input.clone()).map_err(ectx!(ErrorKind::Internal => input)))
                }),
        )
    }
    fn get_account(&self, maybe_token: Option<AuthenticationToken>, account_id: AccountId) -> Box<Future<Item = Option<Account>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        self.db_executor
            .execute(move || accounts_repo.get(account_id).map_err(ectx!(ErrorKind::Internal => account_id)))
    }
    fn update_account(&self, maybe_token: Option<AuthenticationToken>, account_id: AccountId, payload: UpdateAccount) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            payload
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => payload))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute(move || {
                        accounts_repo
                            .update(account_id, payload.clone())
                            .map_err(ectx!(ErrorKind::Internal => account_id, payload))
                    })
                }),
        )
    }
    fn delete_account(&self, maybe_token: Option<AuthenticationToken>, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        self.db_executor
            .execute(move || accounts_repo.delete(account_id).map_err(ectx!(ErrorKind::Internal => account_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repos::*;
    use services::*;
    use tokio_core::reactor::Core;

    // #[test]
    // fn test_create() {
    //     let new_account = NewAccount::default();
    //     let token = new_account.authentication_token.clone();
    //     let auth_service = Arc::new(AuthServiceMock::new(vec![token.clone()]));
    //     let accounts_repo = Arc::new(AccountsRepoMock::default());
    //     let db_executor = DbExecutorMock::default();
    //     let accounts_service = AccountsServiceImpl::new(auth_service, accounts_repo, db_executor);
    //     let mut core = Core::new().unwrap();
    //     let new_account = NewAccount::default();

    //     // creates account
    //     let account = core.run(accounts_service.create_account(new_account));
    //     assert!(account.is_ok());
    // }

    // #[test]
    // fn test_get_by_auth_token() {
    //     let new_account = NewAccount::default();
    //     let token = new_account.authentication_token.clone();
    //     let auth_service = Arc::new(AuthServiceMock::new(vec![token.clone()]));
    //     let accounts_repo = Arc::new(AccountsRepoMock::default());
    //     let db_executor = DbExecutorMock::default();
    //     let accounts_service = AccountsServiceImpl::new(auth_service, accounts_repo, db_executor);
    //     let mut core = Core::new().unwrap();
    //     let new_account = NewAccount::default();

    //     // creates account
    //     let account = core.run(accounts_service.create_account(new_account));
    //     assert!(account.is_ok());

    //     // creates account
    //     let account = core
    //         .run(accounts_service.find_account_by_authentication_token(account.unwrap().authentication_token))
    //         .unwrap();
    //     assert!(account.is_some());
    // }

}
