use std::collections::HashMap;

use chrono::{Duration, Utc};
use diesel;
use diesel::dsl::{any, sum};
use diesel::sql_query;
use diesel::sql_types::Uuid as SqlUuid;
use diesel::sql_types::{BigInt, Numeric, Timestamp, VarChar};

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::accounts::dsl as Accounts;
use schema::transactions::dsl::*;

// 0.001 BTC
const MIN_SIGNIFICANT_SATOSHIS: u128 = 1000;
// 0.01 ETH
const MIN_SIGNIFICANT_ETH: u128 = 500_000_000_000_000;
// 100 STQ
const MIN_SIGNIFICANT_STQ: u128 = 100_000_000_000_000_000_000;

pub trait TransactionsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewTransaction) -> RepoResult<Transaction>;
    fn get(&self, transaction_id: TransactionId) -> RepoResult<Option<Transaction>>;
    fn update_status(&self, blockchain_tx_id: BlockchainTransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction>;
    fn get_by_gid(&self, gid: TransactionId) -> RepoResult<Vec<Transaction>>;
    fn get_by_blockchain_tx(&self, blockchain_tx_id: BlockchainTransactionId) -> RepoResult<Option<Transaction>>;
    fn update_blockchain_tx(&self, transaction_id: TransactionId, blockchain_tx_id: BlockchainTransactionId) -> RepoResult<Transaction>;
    fn get_account_balance(&self, account_id: AccountId, kind: AccountKind) -> RepoResult<Amount>;
    fn get_account_spending(&self, account_id: AccountId, kind: AccountKind, period: Duration) -> RepoResult<Amount>;
    fn get_accounts_balance(&self, auth_user_id: UserId, accounts: &[Account]) -> RepoResult<Vec<AccountWithBalance>>;
    fn list_for_user(&self, user_id_arg: UserId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>>;
    fn list_for_account(&self, account_id: AccountId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>>;
    fn list_groups_for_account_skip_approval(&self, account_id: AccountId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>>;
    fn list_groups_for_user_skip_approval(&self, user_id: UserId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>>;
    fn get_system_balances(&self) -> RepoResult<HashMap<AccountId, (Amount, Amount)>>;
    fn get_blockchain_balances(&self) -> RepoResult<HashMap<(BlockchainAddress, Currency), (Amount, Amount)>>;
    fn get_accounts_for_withdrawal(
        &self,
        value: Amount,
        currency: Currency,
        user_id: UserId,
        total_fee: Amount,
    ) -> RepoResult<Vec<AccountWithBalance>>;
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
struct GidQuery {
    #[sql_type = "SqlUuid"]
    gid: TransactionId,
    #[sql_type = "Timestamp"]
    created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
struct BalanceQuery {
    #[sql_type = "VarChar"]
    address: BlockchainAddress,
    #[sql_type = "VarChar"]
    currency: Currency,
    #[sql_type = "Numeric"]
    sum: Amount,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
struct SystemBalanceQuery {
    #[sql_type = "SqlUuid"]
    id: AccountId,
    #[sql_type = "Numeric"]
    sum: Amount,
}

#[derive(Clone, Default)]
pub struct TransactionsRepoImpl {
    system_user_id: UserId,
}

impl TransactionsRepoImpl {
    pub fn new(system_user_id: UserId) -> Self {
        TransactionsRepoImpl { system_user_id }
    }
}

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

    // SELECT cr_account_id as id, SUM(value) FROM transactions JOIN accounts ON transactions.cr_account_id = accounts.id WHERE accounts.user_id = '00000000-0000-4000-8000-010000000000' AND accounts.kind = 'cr' GROUP BY cr_account_id;
    // SELECT dr_account_id as id, SUM(value) FROM transactions JOIN accounts ON transactions.dr_account_id = accounts.id WHERE accounts.user_id = '00000000-0000-4000-8000-010000000000' AND accounts.kind = 'cr' GROUP BY dr_account_id;

    fn get_system_balances(&self) -> RepoResult<HashMap<AccountId, (Amount, Amount)>> {
        with_tls_connection(|conn| {
            let dr_turnovers: Vec<SystemBalanceQuery> =
                sql_query(
                "SELECT dr_account_id as id, SUM(value) FROM transactions JOIN accounts ON transactions.dr_account_id = accounts.id WHERE accounts.user_id = $1 AND accounts.kind = 'cr' GROUP BY dr_account_id;")
                    .bind::<SqlUuid, _>(self.system_user_id)
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let mut dr_turnovers: HashMap<_, _> = dr_turnovers
                .into_iter()
                .map(|balance_query| (balance_query.id, balance_query.sum))
                .collect();
            let cr_turnovers: Vec<SystemBalanceQuery> =
                sql_query(
                "SELECT cr_account_id as id, SUM(value) FROM transactions JOIN accounts ON transactions.cr_account_id = accounts.id WHERE accounts.user_id = $1 AND accounts.kind = 'cr' GROUP BY cr_account_id;")
                    .bind::<SqlUuid, _>(self.system_user_id)
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let cr_turnovers: HashMap<_, _> = cr_turnovers
                .into_iter()
                .map(|balance_query| (balance_query.id, balance_query.sum))
                .collect();
            let mut res: HashMap<AccountId, (Amount, Amount)> = HashMap::new();
            for key in cr_turnovers.keys() {
                let cr_value = cr_turnovers[key];
                let dr_value = dr_turnovers.get(key).cloned().unwrap_or(Amount::new(0));
                res.insert(key.clone(), (cr_value, dr_value));
                dr_turnovers.remove(key);
            }
            for key in dr_turnovers.keys() {
                res.insert(key.clone(), (Amount::new(0), dr_turnovers[key]));
            }
            Ok(res)
        })
    }

    fn get_blockchain_balances(&self) -> RepoResult<HashMap<(BlockchainAddress, Currency), (Amount, Amount)>> {
        with_tls_connection(|conn| {
            let dr_turnovers: Vec<BalanceQuery> =
                sql_query(
                "SELECT accounts.address, accounts.currency, sums.sum FROM (SELECT dr_account_id, SUM(value) FROM transactions GROUP BY dr_account_id) AS sums INNER JOIN accounts ON accounts.id = sums.dr_account_id WHERE accounts.kind = 'dr'")
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let dr_turnovers: HashMap<_, _> = dr_turnovers
                .into_iter()
                .map(|balance_query| ((balance_query.address, balance_query.currency), balance_query.sum))
                .collect();
            let cr_turnovers: Vec<BalanceQuery> =
                sql_query(
                "SELECT accounts.address, accounts.currency, sums.sum FROM (SELECT cr_account_id, SUM(value) FROM transactions GROUP BY cr_account_id) AS sums INNER JOIN accounts ON accounts.id = sums.cr_account_id WHERE accounts.kind = 'dr'")
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let mut cr_turnovers: HashMap<_, _> = cr_turnovers
                .into_iter()
                .map(|balance_query| ((balance_query.address, balance_query.currency), balance_query.sum))
                .collect();
            let mut res: HashMap<(BlockchainAddress, Currency), (Amount, Amount)> = HashMap::new();
            for key in dr_turnovers.keys() {
                let dr_value = dr_turnovers[key];
                let cr_value = cr_turnovers.get(key).cloned().unwrap_or(Amount::new(0));
                res.insert(key.clone(), (dr_value, cr_value));
                cr_turnovers.remove(key);
            }
            for key in cr_turnovers.keys() {
                res.insert(key.clone(), (Amount::new(0), cr_turnovers[key]));
            }
            Ok(res)
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

    fn get_by_gid(&self, gid_: TransactionId) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            transactions.filter(gid.eq(gid_)).get_results(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => gid_)
            })
        })
    }

    //Todo - add filtering by user
    fn get_by_blockchain_tx(&self, blockchain_tx_id_: BlockchainTransactionId) -> RepoResult<Option<Transaction>> {
        with_tls_connection(|conn| {
            transactions
                .filter(blockchain_tx_id.eq(blockchain_tx_id_.clone()))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => blockchain_tx_id_)
                })
        })
    }

    fn update_status(&self, blockchain_tx_id_: BlockchainTransactionId, transaction_status: TransactionStatus) -> RepoResult<Transaction> {
        with_tls_connection(|conn| {
            let f = transactions.filter(blockchain_tx_id.eq(blockchain_tx_id_.clone()));
            diesel::update(f)
                .set(status.eq(transaction_status))
                .get_result(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => blockchain_tx_id_, transaction_status)
                })
        })
    }
    fn get_account_balance(&self, account_id: AccountId, kind_: AccountKind) -> RepoResult<Amount> {
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

            match kind_ {
                AccountKind::Cr => cr_sum
                    .checked_sub(dr_sum)
                    .ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id)),
                AccountKind::Dr => dr_sum
                    .checked_sub(cr_sum)
                    .ok_or_else(|| ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal => account_id)),
            }
        })
    }
    fn get_account_spending(&self, account_id: AccountId, kind_: AccountKind, period: Duration) -> RepoResult<Amount> {
        with_tls_connection(|conn| {
            let date = Utc::now().naive_utc() - period;
            let txs: Vec<Transaction> = match kind_ {
                AccountKind::Dr => transactions
                    .filter(cr_account_id.eq(account_id))
                    .filter(created_at.ge(date))
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind => account_id)
                    })?,
                AccountKind::Cr => transactions
                    .filter(dr_account_id.eq(account_id))
                    .filter(created_at.ge(date))
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind => account_id)
                    })?,
            };
            txs.into_iter()
                .fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|a| a.checked_add(elem.value)))
                .ok_or(ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal))
        })
    }

    fn list_for_user(&self, user_id_arg: UserId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            let query = transactions.filter(user_id.eq(user_id_arg)).order(id).offset(offset).limit(limit);
            query.get_results(conn).map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => user_id_arg, offset, limit)
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
    fn list_groups_for_account_skip_approval(&self, account_id: AccountId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            let gids: Vec<GidQuery> =
                sql_query(
                "SELECT gid, min(created_at) AS created_at FROM transactions WHERE group_kind <> 'approval' AND (cr_account_id = $1 OR dr_account_id = $1) GROUP BY gid ORDER BY created_at DESC OFFSET $2 LIMIT $3")
                    .bind::<SqlUuid, _>(account_id)
                    .bind::<BigInt, _>(offset)
                    .bind::<BigInt, _>(limit)
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let gids: Vec<_> = gids.into_iter().map(|tuple| tuple.gid).collect();
            transactions
                .filter(gid.eq(any(gids)))
                .order(created_at.desc())
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind)
                })
        })
    }

    fn list_groups_for_user_skip_approval(&self, user_id_: UserId, offset: i64, limit: i64) -> RepoResult<Vec<Transaction>> {
        with_tls_connection(|conn| {
            let gids: Vec<GidQuery> =
                sql_query(
                "SELECT gid, min(created_at) AS created_at FROM transactions WHERE group_kind <> 'approval' AND user_id = $1 GROUP BY gid ORDER BY created_at DESC OFFSET $2 LIMIT $3")
                    .bind::<SqlUuid, _>(user_id_)
                    .bind::<BigInt, _>(offset)
                    .bind::<BigInt, _>(limit)
                    .get_results(conn)
                    .map_err(move |e| {
                        let error_kind = ErrorKind::from(&e);
                        ectx!(try err e, error_kind)
                    })?;
            let gids: Vec<_> = gids.into_iter().map(|tuple| tuple.gid).collect();
            transactions
                .filter(gid.eq(any(gids)))
                .order(created_at.desc())
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind)
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
    fn get_accounts_balance(&self, auth_user_id: UserId, accounts: &[Account]) -> RepoResult<Vec<AccountWithBalance>> {
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

    // Get accounts and balance = how much we should withdraw, net of fees
    // E.g. if fee is 1 STQ and total balance is 10 STQ, then this function will return
    // 9 STQ in balance
    fn get_accounts_for_withdrawal(
        &self,
        mut value_: Amount,
        currency_: Currency,
        user_id_: UserId,
        total_fee: Amount,
    ) -> RepoResult<Vec<AccountWithBalance>> {
        with_tls_connection(|conn| {
            let total_fee = match currency_ {
                // we can drain stq account to 0,
                Currency::Stq => Amount::new(0),
                Currency::Eth => total_fee,
                Currency::Btc => total_fee,
            };
            let minimum_balance = match currency_ {
                Currency::Btc => MIN_SIGNIFICANT_SATOSHIS,
                Currency::Eth => MIN_SIGNIFICANT_ETH,
                // While we don't incur STQ expenses on STQ withdrawals, we could theoretically
                // drain STQ accounts up to 0. But it's not worth doing it, if acc balance < MIN_SIGNIFICANT_STQ
                // i.e. withdrawal will not worth it
                Currency::Stq => MIN_SIGNIFICANT_STQ,
            };
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
            let mut remaining_accounts: HashMap<AccountId, Amount> =
                dr_sum_accounts.into_iter().filter(|(_, sum)| sum.raw() > minimum_balance).collect();

            // filtering accounts with pending transactions
            let pending_transactions: Vec<Transaction> = transactions
                .filter(user_id.eq(&user_id_))
                .filter(currency.eq(currency_))
                .filter(status.eq(TransactionStatus::Pending))
                .get_results(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(try err e, error_kind => value_, currency_, user_id_)
                })?;

            for tx in pending_transactions {
                remaining_accounts.remove(&tx.cr_account_id);
                remaining_accounts.remove(&tx.dr_account_id);
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
                // Note - it may seem counter intuitive that we subtract total_fee from each account
                // rather than from only one. But in reality you will incur the fee on each blockchain
                // transaction.
                // Todo need to design an algorithm of how to handle mutiple accounts withdraw.
                let balance = balance.checked_sub(total_fee).unwrap_or(Amount::new(0));
                if balance >= value_ {
                    r.push(AccountWithBalance {
                        account: acc,
                        balance: value_,
                    });
                    value_ = Amount::new(0);
                    break;
                } else {
                    value_ = value_.checked_sub(balance).expect("Unexpected < 0 value");
                    r.push(AccountWithBalance { account: acc, balance });
                }
            }
            if value_ == Amount::new(0) {
                Ok(r)
            } else {
                Err(ectx!(err ErrorContext::InsufficientWithdrawalFunds, ErrorKind::Internal))
            }
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
            trans.blockchain_tx_id = Some(BlockchainTransactionId::default());

            let transaction = transactions_repo.create(trans)?;
            let transaction_status = TransactionStatus::Done;
            let res = transactions_repo.update_status(transaction.blockchain_tx_id.unwrap(), transaction_status);
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
    //         let res = accounts_repo.get_accounts_for_withdrawal(Amount::new(123), Currency::Eth, user.id);
    //         assert!(res.is_ok());
    //         res
    //     }));
    // }
}
