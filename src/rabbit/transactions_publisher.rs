use futures::future;
use futures_cpupool::CpuPool;
use lapin_futures::channel::{Channel, ExchangeDeclareOptions, QueueDeclareOptions};
use lapin_futures::error::Error as LapinError;
use r2d2::PooledConnection;
use serde_json;
use tokio::net::tcp::TcpStream;

use super::error::*;
use super::r2d2::RabbitConnectionManager;
use super::r2d2::RabbitPool;
use models::*;
use prelude::*;

pub trait TransactionPublisher: Send + Sync + 'static {
    fn publish(&self, tx: TransactionOut) -> Box<Future<Item = (), Error = Error> + Send>;
}

#[derive(Clone)]
pub struct TransactionPublisherImpl {
    rabbit_pool: RabbitPool,
    thread_pool: CpuPool,
}

impl TransactionPublisherImpl {
    pub fn new(rabbit_pool: RabbitPool, thread_pool: CpuPool) -> Self {
        Self { rabbit_pool, thread_pool }
    }

    pub fn init(&self, users: Vec<UserId>) -> impl Future<Item = (), Error = Error> {
        let self_clone = self.clone();
        self.get_channel().and_then(move |channel| self_clone.declare(&channel, users))
    }

    fn get_channel(&self) -> impl Future<Item = PooledConnection<RabbitConnectionManager>, Error = Error> {
        // unresolved at the moment - ideally we want to call get on other thread, since it's blocking
        // on the other hand doing so we escape from the thread that has tokio core reference and
        // therefore cannot do spawns
        // let rabbit_pool = self.rabbit_pool.clone();
        // self.thread_pool
        //     .spawn_fn(move || rabbit_pool.get().map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal)))
        self.rabbit_pool
            .get()
            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
            .into_future()
    }

    fn declare(&self, channel: &Channel<TcpStream>, users: Vec<UserId>) -> impl Future<Item = (), Error = Error> {
        let mut f = vec![];
        let f1: Box<Future<Item = (), Error = LapinError>> = Box::new(channel.exchange_declare(
            "transactions",
            "direct",
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            Default::default(),
        ));
        f.push(f1);
        for user in users {
            let queue_name = format!("transactions_{}", user);
            let f2: Box<Future<Item = (), Error = LapinError>> = Box::new(
                channel
                    .queue_declare(
                        &queue_name,
                        QueueDeclareOptions {
                            durable: true,
                            ..Default::default()
                        },
                        Default::default(),
                    )
                    .map(|_| ()),
            );
            f.push(f2);
            let f3: Box<Future<Item = (), Error = LapinError>> =
                Box::new(channel.queue_bind(&queue_name, "transactions", &queue_name, Default::default(), Default::default()));
            f.push(f3);
        }
        future::join_all(f)
            .map(|_| ())
            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
    }
}

impl TransactionPublisher for TransactionPublisherImpl {
    fn publish(&self, tx: TransactionOut) -> Box<Future<Item = (), Error = Error> + Send> {
        Box::new(
            self.get_channel()
                .and_then(move |channel| {
                    let routing_key = format!("transactions_{}", tx.user_id);
                    let payload = serde_json::to_string(&tx).unwrap().into_bytes();
                    channel
                        .clone()
                        .basic_publish("transactions", &routing_key, payload, Default::default(), Default::default())
                        .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                })
                .map(|_| ()),
        )
    }
}

impl TransactionPublisherImpl {}
