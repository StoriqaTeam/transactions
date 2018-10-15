use std::collections::HashMap;

use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::accounts::dsl::*;

pub trait AccountsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewAccount) -> RepoResult<Account>;
    fn get(&self, account_id: AccountId) -> RepoResult<Option<Account>>;
    fn update(&self, account_id: AccountId, payload: UpdateAccount) -> RepoResult<Account>;
    fn delete(&self, account_id: AccountId) -> RepoResult<Account>;
    fn list_for_user(&self, user_id_arg: UserId, offset: AccountId, limit: i64) -> RepoResult<Vec<Account>>;
    fn get_balance_for_user(&self, user_id: UserId) -> RepoResult<Vec<Balance>>;
    fn inc_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account>;
    fn dec_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account>;
    fn get_by_address(&self, address_: AccountAddress, kind_: AccountKind) -> RepoResult<Account>;
    fn get_with_enough_value(&self, value: Amount, currency: Currency, user_id: UserId) -> RepoResult<Vec<(Account, Amount)>>;
}

#[derive(Clone, Default)]
pub struct AccountsRepoImpl;

impl<'a> AccountsRepo for AccountsRepoImpl {
    fn create(&self, payload: NewAccount) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            diesel::insert_into(accounts)
                .values(payload.clone())
                .get_result::<Account>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }
    fn get(&self, account_id_arg: AccountId) -> RepoResult<Option<Account>> {
        with_tls_connection(|conn| {
            accounts
                .filter(id.eq(account_id_arg))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => account_id_arg)
                })
        })
    }
    fn update(&self, account_id_arg: AccountId, payload: UpdateAccount) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let f = accounts.filter(id.eq(account_id_arg));
            diesel::update(f).set(payload.clone()).get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => account_id_arg, payload)
            })
        })
    }
    fn delete(&self, account_id_arg: AccountId) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let filtered = accounts.filter(id.eq(account_id_arg));
            diesel::delete(filtered).get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => account_id_arg)
            })
        })
    }
    fn list_for_user(&self, user_id_arg: UserId, offset: AccountId, limit: i64) -> RepoResult<Vec<Account>> {
        with_tls_connection(|conn| {
            let query = accounts
                .filter(user_id.eq(user_id_arg))
                .order(id)
                .filter(id.ge(offset))
                .limit(limit);
            query.get_results(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => user_id_arg, offset, limit)
            })
        })
    }
    fn get_balance_for_user(&self, user_id_arg: UserId) -> RepoResult<Vec<Balance>> {
        with_tls_connection(|conn| {
            let query = accounts.filter(user_id.eq(user_id_arg)).order(id);
            let accounts_: Vec<Account> = query.get_results(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, error_kind => user_id_arg)
            })?;
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
        })
    }
    fn inc_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let query = accounts.filter(id.eq(account_id));
            let account: Account = query.get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, error_kind => account_id)
            })?;
            let new_balance_ = amount.checked_add(account.balance);
            let new_balance_ =
                new_balance_.ok_or_else(|| ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => amount, account.balance))?;
            let f = accounts.filter(id.eq(account_id));
            diesel::update(f).set(balance.eq(new_balance_)).get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => account_id, amount)
            })
        })
    }
    fn dec_balance(&self, account_id: AccountId, amount: Amount) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let query = accounts.filter(id.eq(account_id));
            let account: Account = query.get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(try err e, error_kind => account_id)
            })?;
            let new_balance_ = amount.checked_sub(account.balance);
            let new_balance_ =
                new_balance_.ok_or_else(|| ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => amount, account.balance))?;
            let f = accounts.filter(id.eq(account_id));
            diesel::update(f).set(balance.eq(new_balance_)).get_result(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => account_id, amount)
            })
        })
    }
    fn get_by_address(&self, address_: AccountAddress, kind_: AccountKind) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            accounts
                .filter(address.eq(address_.clone()))
                .filter(kind.eq(kind_))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => address_, kind_)
                })
        })
    }
    fn get_with_enough_value(&self, value: Amount, currency_: Currency, user_id_: UserId) -> RepoResult<Vec<(Account, Amount)>> {
        with_tls_connection(|conn| {
            accounts
                .filter(user_id.eq(&user_id_))
                .filter(currency.eq(currency_))
                .filter(kind.eq(AccountKind::Dr))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => value, currency_, user_id_)
                }).map(|accounts_: Vec<Account>| {
                    accounts_
                        .into_iter()
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
                        }).collect()
                })
        })
    }
}

#[cfg(test)]
pub mod tests {
    use diesel::r2d2::ConnectionManager;
    use diesel::PgConnection;
    use futures_cpupool::CpuPool;
    use r2d2;
    use tokio_core::reactor::Core;

    use super::*;
    use config::Config;
    use repos::DbExecutorImpl;

    fn create_executor() -> DbExecutorImpl {
        let config = Config::new().unwrap();
        let manager = ConnectionManager::<PgConnection>::new(config.database.url);
        let db_pool = r2d2::Pool::builder().build(manager).unwrap();
        let cpu_pool = CpuPool::new(1);
        DbExecutorImpl::new(db_pool.clone(), cpu_pool.clone())
    }

    #[test]
    fn accounts_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let res = accounts_repo.create(new_account);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn accounts_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.get(account.id);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn accounts_update() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();

            let payload = UpdateAccount {
                name: Some("test".to_string()),
                ..Default::default()
            };
            let res = accounts_repo.update(account.id, payload);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn accounts_delete() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.delete(account.id);
            assert!(res.is_ok());
            res
        }));
    }
    #[test]
    fn accounts_list() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.list_for_user(user.id, account.id, 1);
            assert!(res.is_ok());
            res
        }));
    }
    #[test]
    fn accounts_inc_balance() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.inc_balance(account.id, Amount::new(123));
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn accounts_dec_balance() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.dec_balance(account.id, Amount::new(123));
            assert!(res.is_ok());
            res
        }));
    }
    #[test]
    fn accounts_get_by_address() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let account = accounts_repo.create(new_account).unwrap();
            let res = accounts_repo.get_by_address(account.address, AccountKind::Cr);
            assert!(res.is_ok());
            res
        }));
    }
    #[test]
    fn accounts_get_min_enough_value() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let accounts_repo = AccountsRepoImpl::default();
        let users_repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            new_account.kind = AccountKind::Dr;
            let account = accounts_repo.create(new_account).unwrap();
            accounts_repo.inc_balance(account.id, Amount::new(123))?;
            let res = accounts_repo.get_with_enough_value(Amount::new(123), Currency::Eth, user.id);
            assert!(res.is_ok());
            res
        }));
    }
}
