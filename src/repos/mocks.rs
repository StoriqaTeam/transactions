use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::accounts::*;
use super::blockchain_transactions::*;
use super::error::*;
use super::executor::{DbExecutor, Isolation};
use super::pending_blockchain_transactions::*;
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
    fn list_for_user(&self, user_id_arg: UserId, _offset: i64, _limit: i64) -> RepoResult<Vec<Account>> {
        let data = self.data.lock().unwrap();
        Ok(data.clone().into_iter().filter(|x| x.user_id == user_id_arg).collect())
    }
    fn get_by_address(&self, address_: AccountAddress, currency_: Currency, kind_: AccountKind) -> RepoResult<Option<Account>> {
        let data = self.data.lock().unwrap();
        let u = data
            .iter()
            .filter(|x| x.address == address_ && x.kind == kind_ && x.currency == currency_)
            .nth(0)
            .cloned();
        Ok(u)
    }

    fn filter_by_address(&self, address_: AccountAddress) -> RepoResult<Vec<Account>> {
        let data = self.data.lock().unwrap();
        let u = data.iter().filter(|x| x.address == address_).cloned().collect();
        Ok(u)
    }

    fn get_by_addresses(&self, addresses: &[AccountAddress], currency_: Currency, kind_: AccountKind) -> RepoResult<Vec<Account>> {
        let addresses: HashSet<_> = addresses.iter().collect();
        let data = self.data.lock().unwrap();
        let u = data
            .iter()
            .filter(|x| addresses.contains(&x.address) && x.kind == kind_ && x.currency == currency_)
            .cloned()
            .collect();
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
            fee: payload.fee,
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, transaction_id: TransactionId) -> RepoResult<Option<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.id == transaction_id).nth(0).cloned())
    }
    fn get_by_blockchain_tx(&self, blockchain_tx_id: BlockchainTransactionId) -> RepoResult<Option<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data
            .iter()
            .filter(|x| x.blockchain_tx_id == Some(blockchain_tx_id.clone()))
            .nth(0)
            .cloned())
    }

    fn update_status(&self, blockchain_tx_id: BlockchainTransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction> {
        let mut data = self.data.lock().unwrap();
        let u = data
            .iter_mut()
            .filter_map(|x| {
                if let Some(x_blockchain_tx_id) = x.blockchain_tx_id.clone() {
                    if x_blockchain_tx_id == blockchain_tx_id {
                        x.status = transaction_status;
                        Some(x)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }).nth(0)
            .cloned();
        Ok(u.unwrap())
    }
    fn list_for_user(&self, user_id: UserId, _offset: i64, _limit: i64) -> RepoResult<Vec<Transaction>> {
        let data = self.data.lock().unwrap();
        Ok(data.clone().into_iter().filter(|x| x.user_id == user_id).collect())
    }
    fn get_accounts_balance(&self, auth_user_id: UserId, accounts: &[Account]) -> RepoResult<Vec<AccountWithBalance>> {
        accounts
            .into_iter()
            .map(|account| {
                let balance = self.get_account_balance(account.id, account.kind)?;
                Ok(AccountWithBalance {
                    account: account.clone(),
                    balance,
                })
            }).collect()
    }
    fn get_account_balance(&self, account_id: AccountId, kind: AccountKind) -> RepoResult<Amount> {
        let data = self.data.lock().unwrap();
        let cr_sum = data
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

        let dr_sum = data
            .clone()
            .iter()
            .fold(Some(Amount::default()), |acc: Option<Amount>, x: &Transaction| {
                if let Some(acc) = acc {
                    if x.dr_account_id == account_id {
                        acc.checked_add(x.value)
                    } else {
                        Some(acc)
                    }
                } else {
                    None
                }
            }).ok_or_else(|| ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id))?;
        match kind {
            AccountKind::Cr => cr_sum
                .checked_sub(dr_sum)
                .ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id)),
            AccountKind::Dr => dr_sum
                .checked_sub(cr_sum)
                .ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id)),
        }
    }
    fn list_for_account(&self, account_id: AccountId, _offset: i64, _limit: i64) -> RepoResult<Vec<Transaction>> {
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

    fn get_with_enough_value(&self, value_: Amount, currency_: Currency, user_id_: UserId) -> RepoResult<Vec<AccountWithBalance>> {
        let data = self.data.lock().unwrap();
        Ok(data
            .clone()
            .into_iter()
            .filter(|x| x.currency == currency_ && x.value > value_ && user_id_ == x.user_id)
            .map(|t| {
                let mut acc = Account::default();
                acc.id = t.cr_account_id;
                AccountWithBalance {
                    account: acc,
                    balance: value_,
                }
            }).collect())
    }
}

#[derive(Clone, Default)]
pub struct PendingBlockchainTransactionsRepoMock {
    data: Arc<Mutex<Vec<PendingBlockchainTransactionDB>>>,
}

impl PendingBlockchainTransactionsRepo for PendingBlockchainTransactionsRepoMock {
    fn create(&self, payload: NewPendingBlockchainTransactionDB) -> RepoResult<PendingBlockchainTransactionDB> {
        let mut data = self.data.lock().unwrap();
        let res = PendingBlockchainTransactionDB {
            hash: payload.hash,
            from_: payload.from_,
            to_: payload.to_,
            currency: payload.currency,
            value: payload.value,
            fee: payload.fee,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.hash == hash_).nth(0).cloned())
    }
    fn delete(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<PendingBlockchainTransactionDB>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.hash == hash_).nth(0).cloned())
    }
}

#[derive(Clone, Default)]
pub struct BlockchainTransactionsRepoMock {
    data: Arc<Mutex<Vec<BlockchainTransactionDB>>>,
}

impl BlockchainTransactionsRepo for BlockchainTransactionsRepoMock {
    fn create(&self, payload: NewBlockchainTransactionDB) -> RepoResult<BlockchainTransactionDB> {
        let mut data = self.data.lock().unwrap();
        let res = BlockchainTransactionDB {
            hash: payload.hash,
            from_: payload.from_,
            to_: payload.to_,
            currency: payload.currency,
            fee: payload.fee,
            block_number: payload.block_number,
            confirmations: payload.confirmations,
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
        };
        data.push(res.clone());
        Ok(res)
    }
    fn get(&self, hash_: BlockchainTransactionId) -> RepoResult<Option<BlockchainTransactionDB>> {
        let data = self.data.lock().unwrap();
        Ok(data.iter().filter(|x| x.hash == hash_).nth(0).cloned())
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
    fn execute_transaction_with_isolation<F, T, E>(&self, _isolation: Isolation, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
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
