use std::sync::Arc;

use futures::future;
use futures_cpupool::CpuPool;
use lapin_futures::channel::{ExchangeDeclareOptions, QueueDeclareOptions};
use lapin_futures::error::Error as LapinError;
use r2d2::PooledConnection;
use serde_json;

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
    channel: Arc<PooledConnection<RabbitConnectionManager>>,
}

impl TransactionPublisherImpl {
    pub fn new(rabbit_pool: RabbitPool, thread_pool: CpuPool) -> Self {
        let channel = Arc::new(rabbit_pool.get().expect("Can not get channel from pool"));
        Self {
            rabbit_pool,
            thread_pool,
            channel,
        }
    }

    pub fn init(&mut self, users: Vec<UserId>) -> impl Future<Item = (), Error = Error> {
        let mut f = vec![];
        let channel = self.channel.clone();
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
        let channel = self.channel.clone();
        let routing_key = format!("transactions_{}", tx.user_id);
        let payload = serde_json::to_string(&tx).unwrap().into_bytes();
        Box::new(
            channel
                .basic_publish("transactions", &routing_key, payload, Default::default(), Default::default())
                .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                .map(|_| ()),
        )
    }
}

#[derive(Clone, Default)]
pub struct TransactionPublisherMock;

impl TransactionPublisher for TransactionPublisherMock {
    fn publish(&self, _tx: TransactionOut) -> Box<Future<Item = (), Error = Error> + Send> {
        Box::new(future::ok(()))
    }
}
