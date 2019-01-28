#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate failure;
extern crate futures;
#[macro_use]
extern crate diesel;
extern crate env_logger;
extern crate futures_cpupool;
extern crate gelf;
extern crate hyper;
extern crate r2d2;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate serde_qs;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate config as config_crate;
extern crate lapin_async;
extern crate lapin_futures;
#[macro_use]
extern crate http_router;
extern crate base64;
extern crate hyper_tls;
extern crate rand;
extern crate regex;
#[macro_use]
extern crate validator_derive;
extern crate num;
extern crate validator;
#[macro_use]
extern crate sentry;
extern crate chrono;
extern crate simplelog;
extern crate tokio;
extern crate tokio_core;
extern crate uuid;

#[macro_use]
mod macros;
pub mod api;
mod client;
mod config;
mod logger;
mod models;
mod prelude;
mod rabbit;
mod repos;
mod schema;
mod sentry_integration;
mod services;
mod utils;

use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use diesel::pg::PgConnection;
use diesel::r2d2::ConnectionManager;
use futures::future::{self, Either};
use futures_cpupool::CpuPool;
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio::timer::{Delay, Timeout};
use tokio_core::reactor::Core;

use self::client::HttpClientImpl;
use self::models::*;
use self::prelude::*;
use self::repos::{
    AccountsRepo, AccountsRepoImpl, BlockchainTransactionsRepo, BlockchainTransactionsRepoImpl, DbExecutor, DbExecutorImpl,
    Error as ReposError, ErrorKind as ReposErrorKind, Isolation, KeyValuesRepoImpl, PendingBlockchainTransactionsRepo,
    PendingBlockchainTransactionsRepoImpl, SeenHashesRepoImpl, StrangeBlockchainTransactionsRepoImpl, TransactionsRepo,
    TransactionsRepoImpl, UsersRepo, UsersRepoImpl,
};
use client::{BlockchainClientImpl, KeysClient, KeysClientImpl};
use config::{Config, System};
use rabbit::{Error, ErrorKind};
use rabbit::{RabbitConnectionManager, TransactionConsumerImpl, TransactionPublisherImpl};
use services::BlockchainFetcher;
use utils::log_error;

pub const DELAY_BEFORE_NACK: u64 = 1000;
pub const DELAY_BEFORE_RECONNECT: u64 = 1000;

pub fn hello() {
    println!("Hello world");
}

pub fn print_config() {
    println!("Parsed config: {:?}", get_config());
}

pub fn start_server() {
    let config = get_config();
    // Prepare sentry integration
    let _sentry = sentry_integration::init(config.sentry.as_ref());
    // Prepare logger
    logger::init(&config);
    upsert_system_accounts();
    let config_clone = config.clone();

    let db_pool = create_db_pool(&config_clone);
    let cpu_pool = CpuPool::new(config_clone.rabbit.thread_pool_size);
    let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
    let fees_accounts_ids = vec![
        config.system.btc_fees_account_id,
        config.system.eth_fees_account_id,
        config.system.stq_fees_account_id,
    ];
    let transactions_repo = Arc::new(TransactionsRepoImpl::new(config_clone.system.system_user_id, fees_accounts_ids));
    let accounts_repo = Arc::new(AccountsRepoImpl);
    let seen_hashes_repo = Arc::new(SeenHashesRepoImpl);
    let blockchain_transactions_repo = Arc::new(BlockchainTransactionsRepoImpl);
    let users_repo = UsersRepoImpl::new(config.system.system_user_id);
    let strange_blockchain_transactions_repo = Arc::new(StrangeBlockchainTransactionsRepoImpl);
    let pending_blockchain_transactions_repo = Arc::new(PendingBlockchainTransactionsRepoImpl);
    let key_values_repo = Arc::new(KeyValuesRepoImpl);
    let client = HttpClientImpl::new(&config_clone);
    let blockchain_client = Arc::new(BlockchainClientImpl::new(&config_clone, client.clone()));
    let keys_client = Arc::new(KeysClientImpl::new(&config_clone, client.clone()));

    debug!("Started creating rabbit connection pool");

    let mut core = tokio_core::reactor::Core::new().expect("Can not create tokio core");
    let rabbit_thread_pool = futures_cpupool::CpuPool::new(config_clone.rabbit.thread_pool_size);
    let rabbit_connection_manager = core
        .run(RabbitConnectionManager::create(&config_clone))
        .map_err(|e| {
            log_error(&e);
        })
        .expect("Can not create rabbit connection manager");
    let rabbit_connection_pool = r2d2::Pool::builder()
        .max_size(config_clone.rabbit.connection_pool_size as u32)
        .test_on_check_out(false)
        .max_lifetime(None)
        .idle_timeout(None)
        .build(rabbit_connection_manager)
        .expect("Cannot build rabbit connection pool");
    debug!("Finished creating rabbit connection pool");
    let mut publisher = TransactionPublisherImpl::new(rabbit_connection_pool.clone(), rabbit_thread_pool.clone());
    core.run(
        db_executor
            .execute(move || -> Result<Vec<UserId>, ReposError> { users_repo.get_all().map(|u| u.into_iter().map(|u| u.id).collect()) })
            .map_err(|e| {
                log_error(&e);
            })
            .and_then(|users| {
                publisher.init(users).map_err(|e| {
                    log_error(&e);
                })
            }),
    )
    .expect("Can not create queue for transactions in rabbit");
    let publisher = Arc::new(publisher);
    let publisher_clone = publisher.clone();
    let fetcher = BlockchainFetcher::new(
        Arc::new(config_clone.clone()),
        transactions_repo,
        accounts_repo,
        seen_hashes_repo,
        blockchain_transactions_repo,
        strange_blockchain_transactions_repo,
        pending_blockchain_transactions_repo,
        key_values_repo,
        blockchain_client,
        keys_client,
        db_executor,
        publisher,
    );
    let consumer = TransactionConsumerImpl::new(rabbit_connection_pool, rabbit_thread_pool);
    thread::spawn(move || {
        let mut core = Runtime::new().expect("Can not create tokio core");
        let consumer_and_chans = core
            .block_on(consumer.subscribe())
            .expect("Can not create subscribers for transactions in rabbit");
        debug!("Subscribing to rabbit");
        let fetcher_clone = fetcher.clone();
        let timeout = config_clone.rabbit.restart_subscription_secs as u64;
        let futures = consumer_and_chans.into_iter().map(move |(stream, channel)| {
            let fetcher_clone = fetcher_clone.clone();
            stream
                .for_each(move |message| {
                    trace!("got message: {}", MessageDelivery::new(message.clone()));
                    let delivery_tag = message.delivery_tag;
                    let channel = channel.clone();
                    let fetcher_future = fetcher_clone.handle_message(message.data);
                    let timeout = Duration::from_secs(timeout);
                    Timeout::new(fetcher_future, timeout).then(move |res| {
                        trace!("send result: {:?}", res);
                        match res {
                            Ok(_) => Either::A(channel.basic_ack(delivery_tag, false)),
                            Err(e) => {
                                let when = if let Some(inner) = e.into_inner() {
                                    log_error(&inner);
                                    Instant::now() + Duration::from_millis(DELAY_BEFORE_NACK)
                                } else {
                                    let err: Error = ectx!(err format_err!("Timeout occured"), ErrorKind::Internal);
                                    log_error(&err);
                                    Instant::now() + Duration::from_millis(0)
                                };
                                Either::B(Delay::new(when).then(move |_| {
                                    channel.basic_nack(delivery_tag, false, true).map_err(|e| {
                                        error!("Error sending nack: {}", e);
                                        e
                                    })
                                }))
                            }
                        }
                    })
                })
                .map_err(|_| ())
        });

        let subscription = future::join_all(futures);
        let _ = core.block_on(subscription);
    });

    api::start_server(config, publisher_clone);
}

fn get_config() -> Config {
    config::Config::new().unwrap_or_else(|e| panic!("Error parsing config: {}", e))
}

pub fn create_user(name: &str) {
    let config = get_config();
    let db_pool = create_db_pool(&config);
    let cpu_pool = CpuPool::new(1);
    let users_repo = UsersRepoImpl::new(config.system.system_user_id);
    let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
    let mut new_user: NewUser = Default::default();
    new_user.name = name.to_string();
    let fut = db_executor.execute(move || -> Result<(), ReposError> {
        let user = users_repo.create(new_user).expect("Failed to create user");
        println!("{}", user.authentication_token.raw());
        Ok(())
    });
    hyper::rt::run(fut.map(|_| ()).map_err(|_| ()));
}

pub fn repair_approval_pending_transaction(id: &str) {
    let config = get_config();
    let db_pool = create_db_pool(&config);
    let cpu_pool = CpuPool::new(1);
    let fees_accounts_ids = vec![
        config.system.btc_fees_account_id,
        config.system.eth_fees_account_id,
        config.system.stq_fees_account_id,
    ];
    let transactions_repo = Arc::new(TransactionsRepoImpl::new(config.system.system_user_id, fees_accounts_ids));
    let blockchain_transactions_repo = BlockchainTransactionsRepoImpl;
    let pending_blockchain_transactions_repo = PendingBlockchainTransactionsRepoImpl;
    let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
    let id = TransactionId::from_str(id).expect("Failed to parse transaction id");
    let fut = db_executor.execute_transaction_with_isolation(Isolation::Serializable, move || -> Result<(), ReposError> {
        let transaction = transactions_repo.get(id).expect("Failed to get transaction");
        let transaction = transaction.expect("Failed to find transaction");
        if transaction.kind != TransactionKind::ApprovalTransfer {
            panic!("Transaction kind is not approval");
        }
        if transaction.status != TransactionStatus::Pending {
            panic!("Transaction status is not pending");
        }
        let hash = transaction.blockchain_tx_id.expect("Failed to get blockchain tx hash");
        let pending_transaction = pending_blockchain_transactions_repo
            .delete(hash.clone())
            .expect("Failed to delete pending blockchain transaction");
        let pending_transaction = pending_transaction.expect("Failed to find pending blockchain transaction");
        let payload: NewBlockchainTransactionDB = pending_transaction.into();
        blockchain_transactions_repo
            .create(payload)
            .expect("Failed to create blockchain transaction");
        let payload = NewTransaction {
            id: TransactionId::generate(),
            gid: transaction.gid,
            user_id: transaction.user_id,
            dr_account_id: transaction.cr_account_id,
            cr_account_id: transaction.dr_account_id,
            currency: transaction.currency,
            value: transaction.value,
            status: TransactionStatus::Done,
            blockchain_tx_id: Some(hash.clone()),
            kind: TransactionKind::Reversal,
            group_kind: TransactionGroupKind::Approval,
            related_tx: Some(id),
            meta: Some(serde_json::Value::String(format!(
                "revers of approval transaction with id {}",
                transaction.id
            ))),
        };
        transactions_repo.create(payload).expect("Failed to create transaction");
        transactions_repo
            .update_status(hash, TransactionStatus::Done)
            .expect("Failed to create transaction");
        Ok(())
    });
    hyper::rt::run(fut.map(|_| ()).map_err(|_| ()));
}

pub fn repair_withdrawal_pending_transaction(id: &str) {
    let config = get_config();
    let db_pool = create_db_pool(&config);
    let cpu_pool = CpuPool::new(1);
    let fees_accounts_ids = vec![
        config.system.btc_fees_account_id,
        config.system.eth_fees_account_id,
        config.system.stq_fees_account_id,
    ];
    let transactions_repo = Arc::new(TransactionsRepoImpl::new(config.system.system_user_id, fees_accounts_ids));
    let blockchain_transactions_repo = BlockchainTransactionsRepoImpl;
    let pending_blockchain_transactions_repo = PendingBlockchainTransactionsRepoImpl;
    let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
    let id = TransactionId::from_str(id).expect("Failed to parse transaction id");
    let fut = db_executor.execute_transaction_with_isolation(Isolation::Serializable, move || -> Result<(), ReposError> {
        let transaction = transactions_repo.get(id).expect("Failed to get transaction");
        let transaction = transaction.expect("Failed to find transaction");
        if transaction.kind != TransactionKind::Withdrawal {
            panic!("Transaction kind is not approval");
        }
        if transaction.status != TransactionStatus::Pending {
            panic!("Transaction status is not pending");
        }

        // get all group transactions
        let all_withdrawal_transactions = transactions_repo
            .get_by_gid(transaction.gid)
            .expect("Failed to find transactions by gid");

        // get fee group transactions
        let fee_transaction = all_withdrawal_transactions
            .iter()
            .filter(|t| t.kind == TransactionKind::Fee)
            .next()
            .cloned()
            .expect("Failed to get fee transaction from gid");

        // get withdrawal transactions in group
        let withdrawal_transactions: Vec<Transaction> = all_withdrawal_transactions
            .into_iter()
            .filter(|t| t.kind == TransactionKind::Withdrawal)
            .collect();

        // get withdrawal transactions total amount
        let total_amount = withdrawal_transactions.iter().fold(Amount::new(0), |acc, elem| {
            acc.checked_add(elem.value).expect("Overflow on collecting total amount")
        });

        // get pending withdrawal transactions in group
        let pending_withdrawal_transactions: Vec<Transaction> = withdrawal_transactions
            .clone()
            .into_iter()
            .filter(|t| t.status == TransactionStatus::Pending)
            .collect();

        // get pending withdrawal transactions total amount
        let pending_total_amount = pending_withdrawal_transactions.iter().fold(Amount::new(0), |acc, elem| {
            acc.checked_add(elem.value).expect("Overflow on collecting pending total amount")
        });

        // get fee reversal amount
        let fee_reversal_amount = if pending_total_amount == total_amount {
            fee_transaction.value
        } else {
            let value = (fee_transaction.value.raw() as f64) * (pending_total_amount.raw() as f64 / total_amount.raw() as f64);
            Amount::new(value as u128)
        };

        let reversal_gid = TransactionId::generate();

        // reverse of withdrawal transactions
        for transaction in pending_withdrawal_transactions {
            let hash = transaction.blockchain_tx_id.expect("Failed to get blockchain tx hash");
            let payload = NewTransaction {
                id: TransactionId::generate(),
                gid: reversal_gid,
                user_id: transaction.user_id,
                dr_account_id: transaction.cr_account_id,
                cr_account_id: transaction.dr_account_id,
                currency: transaction.currency,
                value: transaction.value,
                status: TransactionStatus::Done,
                blockchain_tx_id: Some(hash.clone()),
                kind: TransactionKind::Withdrawal,
                group_kind: TransactionGroupKind::Reversal,
                related_tx: Some(transaction.id),
                meta: Some(serde_json::Value::String(format!(
                    "revers of approval transaction with id {}",
                    transaction.id
                ))),
            };
            transactions_repo.create(payload).expect("Failed to create transaction");
            transactions_repo
                .update_status(hash.clone(), TransactionStatus::Done)
                .expect("Failed to create transaction");

            let pending_transaction = pending_blockchain_transactions_repo
                .delete(hash.clone())
                .expect("Failed to delete pending blockchain transaction");
            let pending_transaction = pending_transaction.expect("Failed to find pending blockchain transaction");
            let payload: NewBlockchainTransactionDB = pending_transaction.into();
            blockchain_transactions_repo
                .create(payload)
                .expect("Failed to create blockchain transaction");
        }

        // reverse of fee transactions
        let payload = NewTransaction {
            id: TransactionId::generate(),
            gid: reversal_gid,
            user_id: fee_transaction.user_id,
            dr_account_id: fee_transaction.cr_account_id,
            cr_account_id: fee_transaction.dr_account_id,
            currency: fee_transaction.currency,
            value: fee_reversal_amount,
            status: TransactionStatus::Done,
            blockchain_tx_id: fee_transaction.blockchain_tx_id,
            kind: TransactionKind::Fee,
            group_kind: TransactionGroupKind::Reversal,
            related_tx: Some(fee_transaction.id),
            meta: Some(serde_json::Value::String(format!(
                "revers of approval fee_transaction with id {}",
                fee_transaction.id
            ))),
        };
        transactions_repo.create(payload).expect("Failed to create transaction");

        Ok(())
    });
    hyper::rt::run(fut.map(|_| ()).map_err(|_| ()));
}

pub fn upsert_system_accounts() {
    let config = get_config();
    let client = HttpClientImpl::new(&config);
    let keys_client = KeysClientImpl::new(&config, client.clone());
    let db_pool = create_db_pool(&config);
    let cpu_pool = CpuPool::new(config.rabbit.thread_pool_size);
    let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);

    let config_clone = config.clone();

    let System {
        btc_transfer_account_id,
        eth_transfer_account_id,
        stq_transfer_account_id,
        btc_liquidity_account_id,
        eth_liquidity_account_id,
        stq_liquidity_account_id,
        btc_fees_account_id,
        eth_fees_account_id,
        stq_fees_account_id,
        ..
    } = config.system.clone();

    let f = db_executor
        .execute(move || {
            let users_repo = UsersRepoImpl::default();
            match users_repo.get(config_clone.system.system_user_id)? {
                Some(system_user) => Ok(system_user),
                None => {
                    let new_user = NewUser {
                        id: config.system.system_user_id,
                        name: "system".to_string(),
                        authentication_token: AuthenticationToken::default(),
                    };
                    users_repo.create(new_user)
                }
            }
        })
        .map_err(|e| {
            log_error(&e.compat());
        })
        .and_then(move |user| {
            let keys_client = keys_client.clone();
            let inputs = [
                (btc_transfer_account_id, user.id, Currency::Btc, "btc_transfer_account"),
                (eth_transfer_account_id, user.id, Currency::Eth, "eth_transfer_account"),
                (stq_transfer_account_id, user.id, Currency::Stq, "stq_transfer_account"),
                (btc_liquidity_account_id, user.id, Currency::Btc, "btc_liquidity_account"),
                (eth_liquidity_account_id, user.id, Currency::Eth, "eth_liquidity_account"),
                (stq_liquidity_account_id, user.id, Currency::Stq, "stq_liquidity_account"),
                (btc_fees_account_id, user.id, Currency::Btc, "btc_fees_account"),
                (eth_fees_account_id, user.id, Currency::Eth, "eth_fees_account"),
                (stq_fees_account_id, user.id, Currency::Stq, "stq_fees_account"),
            ];
            let fs: Vec<_> = inputs
                .into_iter()
                .map(move |(account_id, user_id, currency, name)| {
                    let keys_client = keys_client.clone();
                    let db_executor = db_executor.clone();

                    upsert_system_account(*account_id, *user_id, *currency, name, keys_client, db_executor)
                })
                .collect();
            futures::future::join_all(fs)
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(f);
}

fn upsert_system_account(
    account_id: AccountId,
    user_id: UserId,
    currency: Currency,
    name: &str,
    keys_client: KeysClientImpl,
    db_executor: DbExecutorImpl,
) -> impl Future<Item = (), Error = ()> {
    let name = name.to_string();
    db_executor
        .execute(move || -> Result<(), ReposError> {
            let accounts_repo = AccountsRepoImpl::default();
            match accounts_repo.get(account_id)? {
                Some(_) => Ok(()),
                None => {
                    let input = CreateAccountAddress {
                        id: account_id.inner().clone(),
                        currency,
                    };
                    let mut core = Core::new().unwrap();
                    let account_address_res = core.run(
                        keys_client
                            .create_account_address(input, Role::System)
                            .map_err(ectx!(try ReposErrorKind::Internal)),
                    );
                    if let Err(_) = account_address_res {
                        // just skip if smth is wrong, like account is already created
                        return Ok(());
                    }
                    let account_address = account_address_res.unwrap();
                    let new_cr_account = NewAccount {
                        id: account_id,
                        user_id,
                        currency,
                        address: account_address.clone(),
                        name: Some(name.clone()),
                        kind: AccountKind::Cr,
                    };
                    let dr_account_id = account_id.derive_system_dr_id();
                    let new_dr_account = NewAccount {
                        id: dr_account_id,
                        user_id,
                        currency,
                        address: account_address.clone(),
                        name: Some(format!("{}_deposit", name.clone())),
                        kind: AccountKind::Dr,
                    };
                    accounts_repo.create(new_cr_account)?;
                    accounts_repo.create(new_dr_account)?;
                    Ok(())
                }
            }
        })
        .map(|_| ())
        .map_err(|e| log_error(&e))
}

fn create_db_pool(config: &Config) -> PgPool {
    let database_url = config.database.url.clone();
    let manager = ConnectionManager::<PgConnection>::new(database_url.clone());
    r2d2::Pool::builder()
        .build(manager)
        .unwrap_or_else(|_| panic!("Failed to connect to db with url: {}", database_url))
}
