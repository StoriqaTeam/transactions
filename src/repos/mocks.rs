use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::error::*;
use super::executor::DbExecutor;
use super::types::RepoResult;
use super::users::*;
use super::accounts::*;
use models::*;
use prelude::*;

#[derive(Clone, Default)]
pub struct UsersRepoMock {
    data: Arc<Mutex<Vec<User>>>,
}

impl UsersRepo for UsersRepoMock {
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> Result<Option<User>, Error> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.authentication_token == token).nth(0).cloned())
    }

    fn create(&self, payload: NewUser) -> Result<User, Error> {
        let mut data = self.data.lock().unwrap();
        let res = User {
            id: payload.id,
            name: payload.name,
            authentication_token: payload.authentication_token,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, user_id: UserId) -> RepoResult<Option<User>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == user_id).nth(0).cloned())
    }
    fn update(&self, user_id: UserId, payload: UpdateUser) -> RepoResult<User> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == user_id {
                    if let Some(ref name) = payload.name {
                        x.name = name.clone();
                    }
                    if let Some(ref authentication_token) = payload.authentication_token {
                        x.authentication_token = authentication_token.clone();
                    }
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn delete(&self, user_id: UserId) -> RepoResult<User> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == user_id).nth(0).cloned().unwrap())
    }
}

#[derive(Clone, Default)]
pub struct AccountsRepoMock {
    data: Arc<Mutex<Vec<Account>>>,
}

impl AccountsRepo for AccountsRepoMock {
    fn create(&self, payload: NewAccount) -> Result<Account, Error> {
        let mut data = self.data.lock().unwrap();
        let res = Account {
            id: payload.id,
            name: payload.name,
            user_id: payload.user_id,
            balance: payload.balance,
            currency: payload.currency,
            account_address: payload.account_address,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, account_id: AccountId) -> RepoResult<Option<Account>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == account_id).nth(0).cloned())
    }
    fn update(&self, account_id: AccountId, payload: UpdateAccount) -> RepoResult<Account> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == account_id {
                    if let Some(ref name) = payload.name {
                        x.name = name.clone();
                    }
                    if let Some(ref balance) = payload.balance {
                        x.balance = balance.clone();
                    }
                    if let Some(ref currency) = payload.currency {
                        x.currency = currency.clone();
                    }
                    if let Some(ref account_address) = payload.account_address {
                        x.account_address = account_address.clone();
                    }
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn delete(&self, account_id: AccountId) -> RepoResult<Account> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == account_id).nth(0).cloned().unwrap())
    }
    fn list_for_user(&self, user_id: UserId, offset: AccountId, limit: i64) -> RepoResult<Vec<Account>> {
        let data = self.data.lock().unwrap();
        Ok(data.clone().into_iter().filter(|x| x.user_id == user_id).collect())
    }

}

#[derive(Clone, Default)]
pub struct DbExecutorMock;

impl DbExecutor for DbExecutorMock {
    fn execute<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Send + 'static,
    {
        Box::new(f().into_future())
    }
    fn execute_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Send + 'static,
    {
        Box::new(f().into_future())
    }
    fn execute_test_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail,
    {
        Box::new(f().into_future())
    }
}
