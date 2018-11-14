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

use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use diesel::pg::PgConnection;
use diesel::r2d2::ConnectionManager;
use futures::future::{self, Either};
use futures_cpupool::CpuPool;
use lapin_futures::channel::Channel;
use tokio::net::tcp::TcpStream;
use tokio::prelude::*;
use tokio::timer::{Delay, Timeout};
use tokio_core::reactor::Core;
use uuid::Uuid;

use self::client::HttpClientImpl;
use self::models::*;
use self::prelude::*;
use self::repos::{
    AccountsRepo, AccountsRepoImpl, BlockchainTransactionsRepoImpl, DbExecutor, DbExecutorImpl, Error as ReposError,
    ErrorKind as ReposErrorKind, PendingBlockchainTransactionsRepoImpl, SeenHashesRepoImpl, StrangeBlockchainTransactionsRepoImpl,
    TransactionsRepoImpl, UsersRepo, UsersRepoImpl,
};
use client::{BlockchainClientImpl, KeysClient, KeysClientImpl};
use config::{Config, System};
use rabbit::{ConnectionHooks, RabbitConnectionManager, TransactionConsumerImpl};
use rabbit::{ErrorKind, ErrorSource};
use services::BlockchainFetcher;
use utils::log_error;

pub const DELAY_BEFORE_NACK: u64 = 1000;

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
    thread::spawn(move || {
        let mut core = tokio_core::reactor::Core::new().unwrap();
        let db_pool = create_db_pool(&config_clone);
        let cpu_pool = CpuPool::new(config_clone.rabbit.thread_pool_size);
        let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
        let transactions_repo = Arc::new(TransactionsRepoImpl);
        let accounts_repo = Arc::new(AccountsRepoImpl);
        let seen_hashes_repo = Arc::new(SeenHashesRepoImpl);
        let blockchain_transactions_repo = Arc::new(BlockchainTransactionsRepoImpl);
        let strange_blockchain_transactions_repo = Arc::new(StrangeBlockchainTransactionsRepoImpl);
        let pending_blockchain_transactions_repo = Arc::new(PendingBlockchainTransactionsRepoImpl);
        let client = HttpClientImpl::new(&config_clone);
        let blockchain_client = Arc::new(BlockchainClientImpl::new(&config_clone, client.clone()));
        let keys_client = Arc::new(KeysClientImpl::new(&config_clone, client.clone()));
        let fetcher = BlockchainFetcher::new(
            Arc::new(config_clone.clone()),
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
            strange_blockchain_transactions_repo,
            pending_blockchain_transactions_repo,
            blockchain_client,
            keys_client,
            db_executor,
        );
        debug!("Started creating rabbit connection pool");

        let rabbit_thread_pool = futures_cpupool::CpuPool::new(config_clone.rabbit.thread_pool_size);
        let rabbit_connection_manager = core
            .run(RabbitConnectionManager::create(&config_clone))
            .map_err(|e| {
                log_error(&e);
            }).unwrap();
        let rabbit_connection_pool = r2d2::Pool::builder()
            .max_size(config_clone.rabbit.connection_pool_size as u32)
            .connection_customizer(Box::new(ConnectionHooks))
            .build(rabbit_connection_manager)
            .expect("Cannot build rabbit connection pool");
        debug!("Finished creating rabbit connection pool");
        let publisher = TransactionConsumerImpl::new(rabbit_connection_pool, rabbit_thread_pool);
        loop {
            info!("Subscribing to rabbit");
            let counters = Arc::new(Mutex::new((0usize, 0usize, 0usize, 0usize, 0usize)));
            let counters_clone = counters.clone();
            let consumers_to_close: Arc<Mutex<Vec<(Channel<TcpStream>, String)>>> = Arc::new(Mutex::new(Vec::new()));
            let consumers_to_close_clone = consumers_to_close.clone();
            let fetcher_clone = fetcher.clone();
            let resubscribe_duration = Duration::from_secs(config_clone.rabbit.restart_subscription_secs as u64);
            let subscription = publisher
                .subscribe()
                .and_then(move |consumer_and_chans| {
                    let counters_clone = counters.clone();
                    let futures = consumer_and_chans.into_iter().map(move |(stream, channel)| {
                        let counters_clone = counters_clone.clone();
                        let fetcher_clone = fetcher_clone.clone();
                        let consumers_to_close = consumers_to_close.clone();
                        let mut consumers_to_close_lock = consumers_to_close.lock().unwrap();
                        consumers_to_close_lock.push((channel.clone(), stream.consumer_tag.clone()));
                        drop(consumers_to_close_lock);
                        stream
                            .for_each(move |message| {
                                trace!("got message: {}", MessageDelivery::new(message.clone()));
                                let delivery_tag = message.delivery_tag;
                                let mut counters = counters_clone.lock().unwrap();
                                counters.0 += 1;
                                drop(counters);
                                let counters_clone2 = counters_clone.clone();

                                let channel = channel.clone();
                                fetcher_clone.handle_message(message.data).then(move |res| match res {
                                    Ok(_) => {
                                        let counters_clone = counters_clone2.clone();
                                        let mut counters = counters_clone2.lock().unwrap();
                                        counters.1 += 1;
                                        drop(counters);
                                        Either::A(channel.basic_ack(delivery_tag, false).inspect(move |_| {
                                            let mut counters = counters_clone.lock().unwrap();
                                            counters.2 += 1;
                                            drop(counters);
                                        }))
                                    }
                                    Err(e) => {
                                        let counters_clone = counters_clone2.clone();
                                        let mut counters = counters_clone2.lock().unwrap();
                                        counters.3 += 1;
                                        drop(counters);
                                        log_error(&e);
                                        let when = Instant::now() + Duration::from_millis(DELAY_BEFORE_NACK);
                                        let f = Delay::new(when).then(move |_| {
                                            channel.basic_nack(delivery_tag, false, true).inspect(move |_| {
                                                let mut counters = counters_clone.lock().unwrap();
                                                counters.4 += 1;
                                                drop(counters);
                                            })
                                        });
                                        tokio::spawn(f.map_err(|e| {
                                            error!("Error sending nack: {}", e);
                                        }));
                                        Either::B(future::ok(()))
                                    }
                                })
                            }).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                    });
                    future::join_all(futures)
                }).map_err(|e| {
                    log_error(&e);
                });
            let _ = core
                .run(Timeout::new(subscription, resubscribe_duration).then(move |_| {
                    let counters = counters_clone.lock().unwrap();
                    info!(
                        "Total messages: {}, tried to ack: {}, acked: {}, tried to nack: {}, nacked: {}",
                        counters.0, counters.1, counters.2, counters.3, counters.4
                    );
                    drop(counters);
                    let fs: Vec<_> = consumers_to_close_clone
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .map(|(channel, consumer_tag)| {
                            let mut channel = channel.clone();
                            let consumer_tag = consumer_tag.clone();
                            trace!("Canceling {} with channel `{}`", consumer_tag, channel.id);
                            channel
                                .cancel_consumer(consumer_tag.to_string())
                                .and_then(move |_| channel.close(0, "Cancelled on consumer resubscribe"))
                        }).collect();
                    future::join_all(fs)
                })).map(|_| ())
                .map_err(|e: io::Error| {
                    error!("Error closing consumer {}", e);
                });
        }
    });

    api::start_server(config);
}

fn get_config() -> Config {
    config::Config::new().unwrap_or_else(|e| panic!("Error parsing config: {}", e))
}

pub fn create_user(name: &str) {
    let config = get_config();
    let db_pool = create_db_pool(&config);
    let cpu_pool = CpuPool::new(1);
    let users_repo = UsersRepoImpl;
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
        }).map_err(|e| {
            log_error(&e.compat());
        }).and_then(move |user| {
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
                }).collect();
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
        }).map(|_| ())
        .map_err(|e| log_error(&e))
}

fn create_db_pool(config: &Config) -> PgPool {
    let database_url = config.database.url.clone();
    let manager = ConnectionManager::<PgConnection>::new(database_url.clone());
    r2d2::Pool::builder()
        .build(manager)
        .unwrap_or_else(|_| panic!("Failed to connect to db with url: {}", database_url))
}
