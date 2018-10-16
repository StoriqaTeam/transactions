use diesel;
use diesel::dsl::sum;

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
    fn update_blockchain_tx(&self, transaction_id: TransactionId, blockchain_tx_id: BlockchainTransactionId) -> RepoResult<Transaction>;
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
            let cr_sum: Option<Amount> = transactions
                .filter(cr_account_id.eq(account_id))
                .select(sum(value))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => account_id)
                })?;
            //sum will return null if there are no rows in select statement returned
            let cr_sum = cr_sum.unwrap_or_default();

            let dr_sum: Option<Amount> = transactions
                .filter(dr_account_id.eq(account_id))
                .select(sum(value))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => account_id)
                })?;
            //sum will return null if there are no rows in select statement returned
            let dr_sum = dr_sum.unwrap_or_default();

            cr_sum
                .checked_sub(dr_sum)
                .ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id))
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
    fn update_blockchain_tx(
        &self,
        transaction_id_arg: TransactionId,
        blockchain_tx_id_: BlockchainTransactionId,
    ) -> RepoResult<Transaction> {
        with_tls_connection(|conn| {
            let f = transactions.filter(id.eq(transaction_id_arg));
            diesel::update(f)
                .set(blockchain_tx_id.eq(blockchain_tx_id_.clone()))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => transaction_id_arg, blockchain_tx_id_)
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
    fn transactions_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            let res = transactions_repo.create(trans);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn transactions_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            let transaction = transactions_repo.create(trans)?;
            let res = transactions_repo.get(transaction.id);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn transactions_update_status() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            let transaction = transactions_repo.create(trans)?;
            let transaction_status = TransactionStatus::Done;
            let res = transactions_repo.update_status(transaction.id, transaction_status);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn transactions_list_for_user() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            let transaction = transactions_repo.create(trans)?;
            let res = transactions_repo.list_for_user(user.id, transaction.id, 1);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn transactions_get_account_balance() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            transactions_repo.create(trans)?;
            let res = transactions_repo.get_account_balance(acc1.id);
            assert!(res.is_ok());
            res
        }));
    }
    #[test]
    fn transactions_list_for_account() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            transactions_repo.create(trans)?;
            let res = transactions_repo.list_for_account(acc1.id);
            assert!(res.is_ok());
            res
        }));
    }

    #[test]
    fn transactions_update_blockchain_tx_id() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let users_repo = UsersRepoImpl::default();
        let accounts_repo = AccountsRepoImpl::default();
        let transactions_repo = TransactionsRepoImpl::default();
        let new_user = NewUser::default();
        let _ = core.run(db_executor.execute_test_transaction(move || {
            let user = users_repo.create(new_user)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc1 = accounts_repo.create(new_account)?;
            let mut new_account = NewAccount::default();
            new_account.user_id = user.id;
            let acc2 = accounts_repo.create(new_account)?;

            let mut trans = NewTransaction::default();
            trans.cr_account_id = acc1.id;
            trans.dr_account_id = acc2.id;
            trans.user_id = user.id;
            trans.value = Amount::new(123);

            let transaction = transactions_repo.create(trans)?;
            let tx = BlockchainTransactionId::default();
            let res = transactions_repo.update_blockchain_tx(transaction.id, tx);
            assert!(res.is_ok());
            res
        }));
    }
}
