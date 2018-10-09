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
    fn list_for_user(&self, user_id: UserId, offset: AccountId, limit: i64) -> RepoResult<Vec<Account>>;
}

#[derive(Clone, Default)]
pub struct AccountsRepoImpl;

impl<'a> AccountsRepo for AccountsRepoImpl {
    fn create(&self, payload: NewAccount) -> RepoResult<Account> {
        let payload_clone = payload.clone();
        with_tls_connection(|conn| {
            diesel::insert_into(accounts)
                .values(payload.clone())
                .get_result::<Account>(conn)
                .map_err(move |e| {
                    let kind = ErrorKind::from_diesel(&e);
                    ectx!(err e, kind => payload_clone)
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
                .map_err(ectx!(ErrorKind::Internal))
        })
    }
    fn update(&self, account_id_arg: AccountId, payload: UpdateAccount) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let f = accounts.filter(id.eq(account_id_arg));
            diesel::update(f)
                .set(payload.clone())
                .get_result(conn)
                .map_err(ectx!(ErrorKind::Internal))
        })
    }
    fn delete(&self, account_id_arg: AccountId) -> RepoResult<Account> {
        with_tls_connection(|conn| {
            let filtered = accounts.filter(id.eq(account_id_arg.clone()));
            diesel::delete(filtered).get_result(conn).map_err(ectx!(ErrorKind::Internal))
        })
    }
    fn list_for_user(&self, user_id_arg: UserId, offset: AccountId, limit: i64) -> RepoResult<Vec<Account>> {
        with_tls_connection(|conn| {
            accounts
                .filter(user_id.eq(user_id_arg))
                .filter(id.ge(offset))
                .order(id)
                .limit(limit)
                .get_results(conn)
                .map_err(ectx!(ErrorKind::Internal))
        })
    }
}

#[cfg(test)]
pub mod tests {
    use diesel::r2d2::ConnectionManager;
    use diesel::PgConnection;
    use futures_cpupool::CpuPool;
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
        let repo = AccountsRepoImpl::default();
        let new_user = NewAccount::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.create(new_user)));
        assert!(res.is_ok());
    }

    #[test]
    fn accounts_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewAccount::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.get(new_user.id)));
        assert!(res.is_ok());
    }

    #[test]
    fn accounts_update() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewAccount::default();

        let payload = UpdateAccount {
            name: Some("test".to_string()),
            ..Default::default()
        };
        let res = core.run(db_executor.execute_test_transaction(move || repo.update(new_user.id, payload)));
        assert!(res.is_ok());
    }

    #[test]
    fn accounts_delete() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewAccount::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.delete(new_user.id)));
        assert!(res.is_ok());
    }

    #[test]
    fn accounts_list() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = AccountsRepoImpl::default();
        let new_user = NewUser::default();
        let account_offset = AccountId::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.list_for_user(new_user.id, account_offset, 1)));
        assert!(res.is_ok());
    }
}
