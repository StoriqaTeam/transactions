use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::accounts::*;
use super::error::*;
use super::executor::DbExecutor;
use super::transactions::*;
use super::types::RepoResult;
use super::users::*;
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
        let res: Account = payload.into();
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
                    x.name = payload.name.clone();
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
    fn list_for_user(&self, user_id_arg: UserId, _offset: AccountId, _limit: i64) -> RepoResult<Vec<Account>> {
        let data = self.data.lock().unwrap();
        Ok(data.clone().into_iter().filter(|x| x.user_id == user_id_arg).collect())
    }
    fn get_balance_for_user(&self, user_id: UserId) -> RepoResult<Vec<Balance>> {
        let data = self.data.lock().unwrap();
        let accounts_: Vec<Account> = data.clone().into_iter().filter(|x| x.user_id == user_id).collect();
        let mut hashmap = HashMap::new();
        for account in accounts_ {
            let mut balance_ = hashmap.entry(account.currency).or_insert_with(Amount::default);
            let new_balance = balance_.checked_add(account.balance);
            if let Some(new_balance) = new_balance {
                *balance_ = new_balance;
            } else {
                return Err(ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => balance_, account.balance));
            }
        }
        let balances = hashmap
            .into_iter()
            .map(|(currency_, balance_)| Balance::new(currency_, balance_))
            .collect();
        Ok(balances)
    }
    fn inc_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == account_id {
                    x.balance = x.balance.checked_add(amount).unwrap_or_default();
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn dec_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == account_id {
                    x.balance = x.balance.checked_sub(amount).unwrap_or_default();
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn get_by_address(&self, address_: AccountAddress, kind_: AccountKind) -> RepoResult<Option<Account>> {
        let data = self.data.lock().unwrap();
        let u = data.iter().filter(|x| x.address == address_ && x.kind == kind_).nth(0).cloned();
        Ok(u)
    }
    fn get_with_enough_value(&self, value: Amount, currency: Currency, _user_id: UserId) -> RepoResult<Vec<(Account, Amount)>> {
        let data = self.data.lock().unwrap();
        let u = data
            .clone()
            .into_iter()
            .filter(|x| x.currency == currency && x.balance >= value && x.kind == AccountKind::Dr)
            .scan(value, |remaining_balance, account| match *remaining_balance {
                x if x == Amount::new(0) => None,
                x if x <= account.balance => {
                    let balance_to_sub = *remaining_balance;
                    *remaining_balance = Amount::new(0);
                    Some((account, balance_to_sub))
                }
                x if x > account.balance => {
                    if let Some(new_balance) = remaining_balance.checked_sub(account.balance) {
                        let balance_to_sub = account.balance;
                        *remaining_balance = new_balance;
                        Some((account, balance_to_sub))
                    } else {
                        None
                    }
                }
                _ => None,
            }).collect();
        Ok(u)
    }
}

#[derive(Clone, Default)]
pub struct TransactionsRepoMock {
    data: Arc<Mutex<Vec<Transaction>>>,
}

impl TransactionsRepo for TransactionsRepoMock {
    fn create(&self, payload: NewTransaction) -> Result<Transaction, Error> {
        let mut data = self.data.lock().unwrap();
        let res = Transaction {
            id: payload.id,
            user_id: payload.user_id,
            dr_account_id: payload.dr_account_id,
            cr_account_id: payload.cr_account_id,
            currency: payload.currency,
            value: payload.value,
            status: payload.status,
            blockchain_tx_id: payload.blockchain_tx_id,
            hold_until: payload.hold_until,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, transaction_id: TransactionId) -> RepoResult<Option<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == transaction_id).nth(0).cloned())
    }
    fn update_status(&self, transaction_id: TransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == transaction_id {
                    x.status = transaction_status;
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn list_for_user(&self, user_id: UserId, _offset: TransactionId, _limit: i64) -> RepoResult<Vec<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data.clone().into_iter().filter(|x| x.user_id == user_id).collect())
    }
    fn get_account_balance(&self, account_id: AccountId) -> RepoResult<Amount> {
        let data = self.data.lock().unwrap();
        let sum_cr = data
            .clone()
            .iter()
            .fold(Some(Amount::default()), |acc: Option<Amount>, x: &Transaction| {
                if let Some(acc) = acc {
                    if x.cr_account_id == account_id {
                        acc.checked_add(x.value)
                    } else {
                        Some(acc)
                    }
                } else {
                    None
                }
            }).ok_or_else(|| ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id))?;

        data.clone()
            .iter()
            .fold(Some(sum_cr), |acc: Option<Amount>, x: &Transaction| {
                if let Some(acc) = acc {
                    if x.dr_account_id == account_id {
                        acc.checked_sub(x.value)
                    } else {
                        Some(acc)
                    }
                } else {
                    None
                }
            }).ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id))
    }
    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data
            .clone()
            .into_iter()
            .filter(|x| x.cr_account_id == account_id || x.dr_account_id == account_id)
            .collect())
    }
    fn update_blockchain_tx(&self, transaction_id: TransactionId, blockchain_tx_id_: BlockchainTransactionId) -> RepoResult<Transaction> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if x.id == transaction_id {
                    x.blockchain_tx_id = Some(blockchain_tx_id_.clone());
                    Some(x)
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
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
