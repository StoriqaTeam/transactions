use std::sync::Arc;

use futures::IntoFuture;
use validator::Validate;

use super::auth::AuthService;
use super::error::*;
use client::KeysClient;
use models::*;
use prelude::*;
use repos::{AccountsRepo, DbExecutor};

#[derive(Clone)]
pub struct AccountsServiceImpl<E: DbExecutor> {
    auth_service: Arc<dyn AuthService>,
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    keys_client: Arc<dyn KeysClient>,
}

impl<E: DbExecutor> AccountsServiceImpl<E> {
    pub fn new(auth_service: Arc<AuthService>, accounts_repo: Arc<AccountsRepo>, db_executor: E, keys_client: Arc<dyn KeysClient>) -> Self {
        Self {
            auth_service,
            accounts_repo,
            db_executor,
            keys_client,
        }
    }
}

pub trait AccountsService: Send + Sync + 'static {
    fn create_account(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        input: CreateAccount,
    ) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn get_account(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Option<Account>, Error = Error> + Send>;
    fn update_account(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
        payload: UpdateAccount,
    ) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn delete_account(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn get_accounts_for_user(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<Account>, Error = Error> + Send>;
    fn get_account_balance(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Balance, Error = Error> + Send>;
    fn get_user_balance(&self, token: AuthenticationToken, user_id: UserId) -> Box<Future<Item = Vec<Balance>, Error = Error> + Send>;
}

impl<E: DbExecutor> AccountsService for AccountsServiceImpl<E> {
    fn create_account(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        input: CreateAccount,
    ) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let keys_client = self.keys_client.clone();
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then({
                    let input = input.clone();
                    move |_| {
                        keys_client
                            .create_account_address(input.clone().into())
                            .map_err(ectx!(convert => input))
                    }
                }).and_then(move |address| {
                    db_executor.execute_transaction(move || {
                        let new_account_cr: NewAccount = (input, address).into();
                        let new_account_dr = new_account_cr.create_debit();
                        let users_account = accounts_repo
                            .create(new_account_cr.clone())
                            .map_err(ectx!(try ErrorKind::Internal => new_account_cr))?;
                        accounts_repo
                            .create(new_account_dr.clone())
                            .map_err(ectx!(try ErrorKind::Internal => new_account_dr))?;
                        Ok(users_account)
                    })
                }),
        )
    }
    fn get_account(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Option<Account>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let account = accounts_repo
                    .get(account_id)
                    .map_err(ectx!(try ErrorKind::Internal => account_id))?;
                if let Some(ref account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                }
                Ok(account)
            })
        }))
    }
    fn update_account(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
        payload: UpdateAccount,
    ) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let auth_service = self.auth_service.clone();
        Box::new(
            payload
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => payload))
                .into_future()
                .and_then(move |_| {
                    auth_service.authenticate(token).and_then(move |user| {
                        db_executor.execute_transaction(move || {
                            let account = accounts_repo
                                .update(account_id, payload.clone())
                                .map_err(ectx!(try ErrorKind::Internal => account_id, payload))?;
                            if account.user_id != user.id {
                                return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                            }
                            Ok(account)
                        })
                    })
                }),
        )
    }
    fn delete_account(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute_transaction(move || {
                let account = accounts_repo
                    .delete(account_id)
                    .map_err(ectx!(try ErrorKind::Internal => account_id))?;
                if account.user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                Ok(account)
            })
        }))
    }
    fn get_accounts_for_user(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<Account>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                if user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                accounts_repo
                    .list_for_user(user_id, offset, limit)
                    .map_err(ectx!(ErrorKind::Internal => user_id, offset, limit))
            })
        }))
    }
    fn get_account_balance(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Balance, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let account = accounts_repo
                    .get(account_id)
                    .map_err(ectx!(try ErrorKind::Internal => account_id))?;
                if let Some(account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    Ok(account.into())
                } else {
                    return Err(ectx!(err ErrorContext::NoAccount, ErrorKind::NotFound => account_id));
                }
            })
        }))
    }
    fn get_user_balance(&self, token: AuthenticationToken, user_id: UserId) -> Box<Future<Item = Vec<Balance>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                if user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                accounts_repo
                    .get_balance_for_user(user_id)
                    .map_err(ectx!(ErrorKind::Internal => user_id))
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::*;
    use repos::*;
    use services::*;
    use tokio_core::reactor::Core;

    fn create_account_service(token: AuthenticationToken, user_id: UserId) -> AccountsServiceImpl<DbExecutorMock> {
        let auth_service = Arc::new(AuthServiceMock::new(vec![(token, user_id)]));
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let keys_client = Arc::new(KeysClientMock::default());
        let db_executor = DbExecutorMock::default();
        AccountsServiceImpl::new(auth_service, accounts_repo, db_executor, keys_client)
    }

    #[test]
    fn test_account_create() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        let account = core.run(service.create_account(token, user_id, new_account));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_get() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        let account = core.run(service.get_account(token, new_account.id));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_update() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        core.run(service.create_account(token.clone(), user_id, new_account.clone()))
            .unwrap();

        let mut payload = UpdateAccount::default();
        payload.name = Some("test test test 2acc".to_string());
        let account = core.run(service.update_account(token, new_account.id, payload));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_delete() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;
        core.run(service.create_account(token.clone(), user_id, new_account.clone()))
            .unwrap();

        let account = core.run(service.delete_account(token, new_account.id));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_get_for_users() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        let account = core.run(service.get_accounts_for_user(token, new_account.user_id, 0, 10));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_get_balance() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        core.run(service.create_account(token.clone(), user_id, new_account.clone()))
            .unwrap();

        let account = core.run(service.get_account_balance(token, new_account.id));
        assert!(account.is_ok());
    }
    #[test]
    fn test_account_get_balance_for_users() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let service = create_account_service(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.currency = Currency::Eth;
        new_account.user_id = user_id;

        core.run(service.create_account(token.clone(), user_id, new_account)).unwrap();

        let mut new_account2 = CreateAccount::default();
        new_account2.name = "test tвфвest test acc".to_string();
        new_account2.currency = Currency::Stq;
        new_account2.user_id = user_id;

        core.run(service.create_account(token.clone(), user_id, new_account2)).unwrap();

        let balance = core.run(service.get_user_balance(token, user_id));
        assert!(balance.is_ok());
        let balance = balance.unwrap();
        assert_eq!(balance.len(), 2);
    }

}
