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
extern crate lapin_async;
#[macro_use]
extern crate lapin_futures;
extern crate config as config_crate;
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

use std::io::ErrorKind as IoErrorKind;
use std::sync::Arc;
use std::thread;

use diesel::pg::PgConnection;
use diesel::r2d2::ConnectionManager;
use futures::future::{self, Either};
use futures_cpupool::CpuPool;

use self::models::NewUser;
use self::prelude::*;
use self::repos::{
    BlockchainTransactionsRepoImpl, DbExecutor, DbExecutorImpl, Error as ReposError, SeenHashesRepoImpl, TransactionsRepoImpl, UsersRepo,
    UsersRepoImpl,
};
use config::Config;
use rabbit::{ErrorKind, ErrorSource};
use rabbit::{RabbitConnectionManager, TransactionConsumerImpl};
use services::BlockchainWorkerImpl;
use utils::log_error;

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
        let cpu_pool = CpuPool::new(1);
        let db_executor = DbExecutorImpl::new(db_pool, cpu_pool);
        let transactions_repo = Arc::new(TransactionsRepoImpl);
        let seen_hashes_repo = Arc::new(SeenHashesRepoImpl);
        let blockchain_transactions_repo = Arc::new(BlockchainTransactionsRepoImpl);
        let worker = BlockchainWorkerImpl::new(transactions_repo, seen_hashes_repo, blockchain_transactions_repo, db_executor);
        debug!("Started creating rabbit connection pool");
        let rabbit_thread_pool = futures_cpupool::CpuPool::new(config_clone.rabbit.thread_pool_size);
        let f = RabbitConnectionManager::create(&config_clone)
            .and_then(move |rabbit_connection_manager| {
                let rabbit_connection_pool = r2d2::Pool::builder()
                    .max_size(config_clone.rabbit.connection_pool_size as u32)
                    .build(rabbit_connection_manager)
                    .expect("Cannot build rabbit connection pool");
                debug!("Finished creating rabbit connection pool");
                let publisher = TransactionConsumerImpl::new(rabbit_connection_pool, rabbit_thread_pool);
                let publisher_clone = publisher.clone();
                let worker_clone = worker.clone();
                publisher.init().and_then(move |consumers| {
                    let futures = consumers.into_iter().map(move |stream| {
                        let publisher_clone = publisher_clone.clone();
                        let worker_clone = worker_clone.clone();
                        stream
                            .for_each(move |message| {
                                debug!("got message: {:?}", message);
                                let delivery_tag = message.delivery_tag;
                                let worker_clone = worker_clone.clone();
                                let publisher_clone = publisher_clone.clone();
                                String::from_utf8(message.data)
                                    .map_err(|_| IoErrorKind::Other.into())
                                    .into_future()
                                    .and_then(|s| serde_json::from_str(&s).map_err(|_| IoErrorKind::Other.into()).into_future())
                                    .and_then(move |blockchain_transaction| worker_clone.work(blockchain_transaction))
                                    .then(move |res| match res {
                                        Ok(_) => Either::A(publisher_clone.ack(delivery_tag)),
                                        Err(_) => Either::B(publisher_clone.nack(delivery_tag)),
                                    })
                            }).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                    });
                    future::join_all(futures)
                })
            }).map_err(|e| {
                log_error(&e);
            });
        let _ = core.run(f.and_then(|_| futures::future::empty::<(), ()>()));
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
