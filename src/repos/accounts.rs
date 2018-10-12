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
    fn get_min_enough_value(&self, value: Amount, currency: Currency, user_id: UserId) -> RepoResult<Account>;
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
            let filtered = accounts.filter(id.eq(account_id_arg.clone()));
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
    fn get_min_enough_value(&self, value: Amount, currency_: Currency, user_id_: UserId) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            accounts
                .filter(user_id.eq(&user_id_))
                .filter(currency.eq(currency_.clone()))
                .order(balance)
                .filter(balance.ge(value))
                .filter(kind.eq(AccountKind::Dr))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => value, currency_, user_id_)
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

    #[ignore]
    #[test]
    fn accounts_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewAccount::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.create(new_user)));
        println!("{:?}", res);
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn accounts_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let account_id = AccountId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.get(account_id)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn accounts_update() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let account_id = AccountId::generate();

        let payload = UpdateAccount {
            name: Some("test".to_string()),
            ..Default::default()
        };
        let res = core.run(db_executor.execute_test_transaction(move || repo.update(account_id, payload)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn accounts_delete() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let _new_user = NewAccount::default();
        let account_id = AccountId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.delete(account_id)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn accounts_list() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewUser::default();
        let account_offset = AccountId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.list_for_user(new_user.id, account_offset, 1)));
        assert!(res.is_ok());
    }
}
