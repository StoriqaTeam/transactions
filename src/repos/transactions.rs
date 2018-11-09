use std::collections::HashMap;

use diesel;
use diesel::dsl::{any, sum};
use diesel::sql_query;
use diesel::sql_types::Uuid as SqlUuid;
use diesel::sql_types::VarChar;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::accounts::dsl as Accounts;
use schema::transactions::dsl::*;
use schema::tx_groups::dsl as TxGroupsDsl;

pub trait TransactionsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewTransaction) -> RepoResult<Transaction>;
    fn get(&self, transaction_id: TransactionId) -> RepoResult<Option<Transaction>>;
    fn list(&self, transaction_ids: &[TransactionId]) -> RepoResult<Vec<Transaction>>;
    fn get_with_enough_value(&self, value: Amount, currency: Currency, user_id: UserId) -> RepoResult<Vec<AccountWithBalance>>;
    fn list_by_tx_group_id(&self, tx_group_id: TransactionId) -> RepoResult<Vec<Transaction>>;
    fn list_balances_for_accounts(&self, auth_user_id: UserId, accounts: &[Account]) -> RepoResult<Vec<AccountWithBalance>>;
    fn list_for_account(&self, account_id: AccountId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>>;
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

    fn list(&self, transaction_ids: &[TransactionId]) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            transactions.
                filter(id.eq(any(transaction_ids)))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => transaction_ids)
                })
        })
    }

    fn list_by_tx_group_id(&self, tx_group_id_: TransactionId) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            transactions.filter(tx_group_id.eq(tx_group_id_)).get_results(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => tx_group_id_)
            })
        })
    }

    fn list_for_account(&self, account_id: AccountId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            transactions
                .filter(dr_account_id.eq(account_id).or(cr_account_id.eq(account_id)))
                .order(created_at.desc())
                .offset(offset)
                .limit(limit)
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => account_id)
                })
        })
    }
    fn list_balances_for_accounts(&self, auth_user_id: UserId, accounts: &[Account]) -> RepoResult<Vec<AccountWithBalance>> {
        // assert all accounts in the same workspace with authed user
        with_tls_connection(|conn| {
            let ids: Vec<_> = accounts.into_iter().map(|acc| acc.id).collect();
            let txs = transactions
                .filter(dr_account_id.eq(any(ids.clone())).or(cr_account_id.eq(any(ids))))
                .get_results::<Transaction>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => auth_user_id, accounts)
                })?;
            let txs_grouped_initial: HashMap<AccountId, Vec<Transaction>> = accounts.into_iter().map(|acc| (acc.id, vec![])).collect();
            let txs_grouped: HashMap<AccountId, Vec<Transaction>> = txs.into_iter().fold(txs_grouped_initial, |mut acc, elem| {
                acc.entry(elem.dr_account_id).and_modify(|txs| txs.push(elem.clone()));
                acc.entry(elem.cr_account_id).and_modify(|txs| txs.push(elem));
                acc
            });
            accounts
                .into_iter()
                .map(|account| {
                    let plus = txs_grouped
                        .get(&account.id)
                        .unwrap()
                        .into_iter()
                        .filter(|tx| match account.kind {
                            AccountKind::Cr => tx.cr_account_id == account.id,
                            AccountKind::Dr => tx.dr_account_id == account.id,
                        }).fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|val| val.checked_add(elem.value)))
                        .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
                    let minus = txs_grouped
                        .get(&account.id)
                        .unwrap()
                        .into_iter()
                        .filter(|tx| match account.kind {
                            AccountKind::Cr => tx.dr_account_id == account.id,
                            AccountKind::Dr => tx.cr_account_id == account.id,
                        }).fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|val| val.checked_add(elem.value)))
                        .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
                    let balance = plus
                        .checked_sub(minus)
                        .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
                    Ok(AccountWithBalance {
                        account: account.clone(),
                        balance,
                    })
                }).collect()
        })
    }

    fn get_with_enough_value(&self, mut value_: Amount, currency_: Currency, user_id_: UserId) -> RepoResult<Vec<AccountWithBalance>> {
        with_tls_connection(|conn| {
            // get all dr accounts
            let dr_sum_accounts: Vec<TransactionSum> =
                sql_query(
                "SELECT SUM(value) as sum, dr_account_id as account_id FROM transactions WHERE currency = $1 AND user_id = $2 GROUP BY dr_account_id")
                    .bind::<VarChar, _>(currency_)
                    .bind::<SqlUuid, _>(user_id_)
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let mut dr_sum_accounts = dr_sum_accounts
                .into_iter()
                .map(|r: TransactionSum| (r.account_id, r.sum))
                .collect::<HashMap<AccountId, Amount>>();

            // get all cr accounts
            let cr_sum_accounts: Vec<TransactionSum> = sql_query(
                "SELECT SUM(value) as sum, cr_account_id as account_id FROM transactions WHERE currency = $1 AND user_id = $2 GROUP BY cr_account_id")
                .bind::<VarChar, _>(currency_)
                .bind::<SqlUuid, _>(user_id_)
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind)
                })?;

            // get accounts balance
            for tr in cr_sum_accounts {
                if let Some(dr_sum) = dr_sum_accounts.get_mut(&tr.account_id) {
                    *dr_sum = dr_sum.checked_sub(tr.sum).unwrap_or_default();
                }
            }

            // filtering accounts with empty balance
            let mut remaining_accounts: HashMap<AccountId, Amount> = dr_sum_accounts.into_iter().filter(|(_, sum)| sum.raw() > 0).collect();

            // filtering accounts with pending transactions
            let pending_accounts: Vec<Transaction> = transactions
                .filter(user_id.eq(&user_id_))
                .filter(currency.eq(currency_))
                .filter(status.eq(TransactionStatus::Pending))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => value_, currency_, user_id_)
                })?;

            for acc in pending_accounts {
                remaining_accounts.remove(&acc.cr_account_id);
                remaining_accounts.remove(&acc.dr_account_id);
            }

            let res_account_ids: Vec<AccountId> = remaining_accounts.keys().cloned().collect();

            // filtering accounts only DR
            let res_accounts: Vec<Account> = Accounts::accounts
                .filter(Accounts::id.eq_any(res_account_ids))
                .filter(Accounts::kind.eq(AccountKind::Dr))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind)
                })?;

            let res_accounts: Vec<(Account, Amount)> = res_accounts
                .into_iter()
                .map(|acc| {
                    let balance = remaining_accounts.get(&acc.id).cloned().unwrap_or_default();
                    (acc, balance)
                }).collect();

            // calculating accounts to take
            let mut r = vec![];
            for (acc, balance) in res_accounts {
                if balance >= value_ {
                    r.push(AccountWithBalance {
                        account: acc,
                        balance: value_,
                    });
                } else {
                    if let Some(new_balance) = value_.checked_sub(balance) {
                        value_ = new_balance;
                        r.push(AccountWithBalance {
                            account: acc,
                            balance: value_,
                        });
                    }
                }
            }

            Ok(r)
        })
    }

}

impl TransactionsRepoImpl {
    fn get_pending_accounts(&self) -> RepoResult<Vec<Account>> {
        with_tls_connection(|conn| {
            let groups = TxGroupsDsl::tx_groups
                .filter(TxGroupsDsl::status.eq(TransactionStatus::Pending))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind)
                })?;
            let tx_ids = groups.into_iter().filter_map(|tx_group| {
                match tx_group.kind {
                    TxGroupKind::Withdrawal => tx_group.tx_1,
                    TxGroupKind::WithdrawalMulti => tx_group.tx_2,
                    _ => None
                }
            }).map(|tx_group| tx_group.tx_1)
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

    // #[test]
    // fn transactions_update_status() {
    //     let mut core = Core::new().unwrap();
    //     let db_executor = create_executor();
    //     let users_repo = UsersRepoImpl::default();
    //     let accounts_repo = AccountsRepoImpl::default();
    //     let transactions_repo = TransactionsRepoImpl::default();
    //     let new_user = NewUser::default();
    //     let _ = core.run(db_executor.execute_test_transaction(move || {
    //         let user = users_repo.create(new_user)?;
    //         let mut new_account = NewAccount::default();
    //         new_account.user_id = user.id;
    //         let acc1 = accounts_repo.create(new_account)?;
    //         let mut new_account = NewAccount::default();
    //         new_account.user_id = user.id;
    //         let acc2 = accounts_repo.create(new_account)?;

    //         let mut trans = NewTransaction::default();
    //         trans.cr_account_id = acc1.id;
    //         trans.dr_account_id = acc2.id;
    //         trans.user_id = user.id;
    //         trans.value = Amount::new(123);
    //         trans.blockchain_tx_id = Some(BlockchainTransactionId::default());

    //         let transaction = transactions_repo.create(trans)?;
    //         let transaction_status = TransactionStatus::Done;
    //         let res = transactions_repo.update_status(transaction.blockchain_tx_id.unwrap(), transaction_status);
    //         assert!(res.is_ok());
    //         res
    //     }));
    // }

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

            let _ = transactions_repo.create(trans)?;
            let res = transactions_repo.list_for_user(user.id, 0, 1);
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
            let res = transactions_repo.get_account_balance(acc1.id, AccountKind::Cr);
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
            let res = transactions_repo.list_for_account(acc1.id, 0, 10);
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
    // #[test]
    // fn transactions_get_min_enough_value() {
    //     let mut core = Core::new().unwrap();
    //     let db_executor = create_executor();
    //     let accounts_repo = AccountsRepoImpl::default();
    //     let users_repo = UsersRepoImpl::default();
    //     let transactions_repo = TransactionsRepoImpl::default();
    //     let new_user = NewUser::default();
    //     let _ = core.run(db_executor.execute_test_transaction(move || {
    //         let user = users_repo.create(new_user)?;
    //         let mut new_account = NewAccount::default();
    //         new_account.user_id = user.id;
    //         let acc1 = accounts_repo.create(new_account)?;
    //         let mut new_account = NewAccount::default();
    //         new_account.user_id = user.id;
    //         let acc2 = accounts_repo.create(new_account)?;

    //         let mut trans = NewTransaction::default();
    //         trans.cr_account_id = acc1.id;
    //         trans.dr_account_id = acc2.id;
    //         trans.user_id = user.id;
    //         trans.value = Amount::new(123);

    //         let _ = transactions_repo.create(trans).unwrap();
    //         let res = accounts_repo.get_with_enough_value(Amount::new(123), Currency::Eth, user.id);
    //         assert!(res.is_ok());
    //         res
    //     }));
    // }
}
