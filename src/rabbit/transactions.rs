use std::io::Error as IoError;

use futures::future;
use futures_cpupool::CpuPool;
use lapin_futures::channel::BasicConsumeOptions;
use lapin_futures::channel::Channel;
use lapin_futures::channel::QueueDeclareOptions;
use lapin_futures::consumer::Consumer;
use lapin_futures::types::FieldTable;
use r2d2::PooledConnection;
use tokio::net::tcp::TcpStream;

use super::error::*;
use super::r2d2::RabbitConnectionManager;
use super::r2d2::RabbitPool;
use prelude::*;

#[derive(Clone)]
pub struct TransactionConsumerImpl {
    rabbit_pool: RabbitPool,
    thread_pool: CpuPool,
}

impl TransactionConsumerImpl {
    pub fn new(rabbit_pool: RabbitPool, thread_pool: CpuPool) -> Self {
        Self { rabbit_pool, thread_pool }
    }

    pub fn init(&self) -> impl Future<Item = Vec<Consumer<TcpStream>>, Error = Error> {
        let self_clone = self.clone();
        self.get_channel().and_then(move |channel| self_clone.declare(&channel))
    }

    pub fn ack(&self, delivery_tag: u64) -> impl Future<Item = (), Error = IoError> {
        self.get_channel()
            .map_err(From::from)
            .and_then(move |channel| channel.basic_ack(delivery_tag, false))
    }

    pub fn nack(&self, delivery_tag: u64) -> impl Future<Item = (), Error = IoError> {
        self.get_channel()
            .map_err(From::from)
            .and_then(move |channel| channel.basic_nack(delivery_tag, false, true))
    }

    fn get_channel(&self) -> impl Future<Item = PooledConnection<RabbitConnectionManager>, Error = Error> {
        let rabbit_pool = self.rabbit_pool.clone();
        self.thread_pool
            .spawn_fn(move || rabbit_pool.get().map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal)))
    }

    fn declare(&self, channel: &Channel<TcpStream>) -> impl Future<Item = Vec<Consumer<TcpStream>>, Error = Error> {
        let self_clone = self.clone();
        let btc_transactions: Box<Future<Item = Consumer<TcpStream>, Error = Error>> = Box::new(
            channel
                .queue_declare(
                    "btc_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                .and_then(move |queue| {
                    self_clone.get_channel().and_then(move |channel| {
                        channel
                            .basic_consume(&queue, "", BasicConsumeOptions::default(), FieldTable::new())
                            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                    })
                }),
        );
        let self_clone = self.clone();
        let stq_transactions: Box<Future<Item = Consumer<TcpStream>, Error = Error>> = Box::new(
            channel
                .queue_declare(
                    "stq_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                .and_then(move |queue| {
                    self_clone.get_channel().and_then(move |channel| {
                        channel
                            .basic_consume(&queue, "", BasicConsumeOptions::default(), FieldTable::new())
                            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                    })
                }),
        );
        let self_clone = self.clone();
        let eth_transactions: Box<Future<Item = Consumer<TcpStream>, Error = Error>> = Box::new(
            channel
                .queue_declare(
                    "eth_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                .and_then(move |queue| {
                    self_clone.get_channel().and_then(move |channel| {
                        channel
                            .basic_consume(&queue, "", BasicConsumeOptions::default(), FieldTable::new())
                            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                    })
                }),
        );
        future::join_all(vec![btc_transactions, stq_transactions, eth_transactions]).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
    }
}
