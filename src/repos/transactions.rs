use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::transactions::dsl::*;

pub trait TransactionsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewTransaction) -> RepoResult<Transaction>;
    fn get(&self, transaction_id: TransactionId) -> RepoResult<Option<Transaction>>;
    fn update_status(&self, transaction_id: TransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction>;
    fn get_account_balance(&self, account_id: AccountId) -> RepoResult<Amount>;
    fn list_for_user(&self, user_id_arg: UserId, offset: TransactionId, limit: i64) -> RepoResult<Vec<Transaction>>;
    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<Transaction>>;
}

#[derive(Clone, Default)]
pub struct TransactionsRepoImpl;

impl TransactionsRepo for TransactionsRepoImpl {
    fn create(&self, payload: NewTransaction) -> RepoResult<Transaction> {
        with_tls_connection(|conn| {
            diesel::insert_into(transactions)
                .values(payload.clone())
                .get_result::<Transaction>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }
    fn get(&self, transaction_id_arg: TransactionId) -> RepoResult<Option<Transaction>> {
        with_tls_connection(|conn| {
            transactions
                .filter(id.eq(transaction_id_arg))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => transaction_id_arg)
                })
        })
    }
    fn update_status(&self, transaction_id_arg: TransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction> {
        with_tls_connection(|conn| {
            let f = transactions.filter(id.eq(transaction_id_arg));
            diesel::update(f)
                .set(status.eq(transaction_status))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => transaction_id_arg, transaction_status)
                })
        })
    }
    fn get_account_balance(&self, account_id: AccountId) -> RepoResult<Amount> {
        with_tls_connection(|conn| {
            let transactions_ = transactions
                .filter(dr_account_id.eq(account_id).or(cr_account_id.eq(account_id)))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => account_id)
                })?;

            transactions_
                .iter()
                .fold(Some(Amount::default()), |acc: Option<Amount>, x: &Transaction| {
                    if let Some(acc) = acc {
                        if x.cr_account_id == account_id {
                            acc.checked_add(x.value)
                        } else {
                            acc.checked_sub(x.value)
                        }
                    } else {
                        None
                    }
                }).ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id))
        })
    }
    fn list_for_user(&self, user_id_arg: UserId, offset: TransactionId, limit: i64) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            let query = transactions
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
    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            transactions
                .filter(dr_account_id.eq(account_id).or(cr_account_id.eq(account_id)))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => account_id)
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
    fn transactions_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = TransactionsRepoImpl::default();
        let new_user = NewTransaction::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.create(new_user)));
        println!("{:?}", res);
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn transactions_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = TransactionsRepoImpl::default();
        let transaction_id = TransactionId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.get(transaction_id)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn transactions_update() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = TransactionsRepoImpl::default();
        let transaction_id = TransactionId::generate();
        let transaction_status = TransactionStatus::Done;
        let res = core.run(db_executor.execute_test_transaction(move || repo.update_status(transaction_id, transaction_status)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn transactions_list_for_user() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let transaction_offset = TransactionId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.list_for_user(new_user.id, transaction_offset, 1)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn transactions_get_account_balance() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = TransactionsRepoImpl::default();
        let account_id = AccountId::generate();
        let res = core.run(db_executor.execute_test_transaction(move || repo.get_account_balance(account_id)));
        assert!(res.is_ok());
    }
}
