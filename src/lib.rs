#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate failure;
extern crate futures;
#[macro_use]
extern crate diesel;
extern crate futures_cpupool;
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
extern crate tokio;
extern crate tokio_core;
extern crate uuid;

#[macro_use]
mod macros;
pub mod api;
mod client;
mod config;
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
use lapin_futures::consumer::ConsumerSub;
use tokio::net::tcp::TcpStream;
use tokio::prelude::*;
use tokio::timer::{Delay, Timeout};

use self::models::{MessageDelivery, NewUser};
use self::prelude::*;
use self::repos::{
    AccountsRepoImpl, BlockchainTransactionsRepoImpl, DbExecutor, DbExecutorImpl, Error as ReposError, SeenHashesRepoImpl,
    TransactionsRepoImpl, UsersRepo, UsersRepoImpl,
};
use config::Config;
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
        let fetcher = BlockchainFetcher::new(
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
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
            let counters = Arc::new(Mutex::new((0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize)));
            let counters_clone = counters.clone();
            let consumers_to_close: Arc<Mutex<Vec<(Channel<TcpStream>, String, ConsumerSub)>>> = Arc::new(Mutex::new(Vec::new()));
            let consumers_to_close_clone = consumers_to_close.clone();
            let fetcher_clone = fetcher.clone();
            let resubscribe_duration = Duration::from_secs(config_clone.rabbit.restart_subscription_secs as u64);
            let ack_timeout_duration = Duration::from_secs(config_clone.rabbit.ack_timeout_secs as u64);
            let subscription = publisher
                .subscribe()
                .and_then(move |consumer_and_chans| {
                    let counters_clone = counters.clone();
                    let futures = consumer_and_chans.into_iter().map(move |(stream, channel)| {
                        let counters_clone = counters_clone.clone();
                        let fetcher_clone = fetcher_clone.clone();
                        let consumers_to_close = consumers_to_close.clone();
                        let mut consumers_to_close_lock = consumers_to_close.lock().unwrap();
                        consumers_to_close_lock.push((channel.clone(), stream.consumer_tag.clone(), stream.subscriber()));
                        drop(consumers_to_close_lock);
                        stream
                            .for_each(move |message| {
                                trace!("got message: {}", MessageDelivery::new(message.clone()));
                                let delivery_tag = message.delivery_tag;
                                let done = Arc::new(Mutex::new(false));
                                let done_clone = done.clone();
                                let when = Instant::now() + ack_timeout_duration;
                                let timeout_channel = channel.clone();
                                let counters_clone4 = counters_clone.clone();
                                let ensure_response_f = Delay::new(when).then(move |_| {
                                    let done = done_clone.lock().unwrap();
                                    if !*done {
                                        {
                                            let mut counters = counters_clone4.lock().unwrap();
                                            counters.5 += 1;
                                        }
                                        Either::A(timeout_channel.basic_nack(delivery_tag, false, true).inspect(move |_| {
                                            let mut counters = counters_clone4.lock().unwrap();
                                            counters.6 += 1;
                                        }))
                                    } else {
                                        Either::B(future::ok(()))
                                    }
                                });
                                tokio::spawn(ensure_response_f.map_err(|e| {
                                    error!("Error sending nack on ack timeout consumer: {}", e);
                                }));

                                let mut counters = counters_clone.lock().unwrap();
                                counters.0 += 1;
                                drop(counters);
                                let counters_clone2 = counters_clone.clone();

                                let channel = channel.clone();
                                fetcher_clone.process(message.data).then(move |res| {
                                    let mut done = done.lock().unwrap();
                                    *done = true;
                                    drop(done);
                                    match res {
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
                        "Total messages: {}, tried to ack: {}, acked: {}, tried to nack: {}, nacked: {}, tried to (ack timeout): {}, nacked (ack timeout): {}",
                        counters.0, counters.1, counters.2, counters.3, counters.4, counters.5, counters.6
                    );
                    drop(counters);
                    let fs: Vec<_> = consumers_to_close_clone
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .map(|tuple| {
                            info!("Canceling {} with channel `{}` and sub: {:?}", tuple.1, tuple.0.id, tuple.2);
                            tuple.0.cancel_consumer(tuple.1.to_string())
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

fn create_db_pool(config: &Config) -> PgPool {
    let database_url = config.database.url.clone();
    let manager = ConnectionManager::<PgConnection>::new(database_url.clone());
    r2d2::Pool::builder()
        .build(manager)
        .unwrap_or_else(|_| panic!("Failed to connect to db with url: {}", database_url))
}
