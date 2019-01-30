use std::sync::Arc;

use futures::future;
use lapin_futures::channel::{Channel, ExchangeDeclareOptions, QueueDeclareOptions};
use lapin_futures::error::Error as LapinError;
use serde_json;
use tokio::net::tcp::TcpStream;

use super::error::*;
use models::*;
use prelude::*;

pub trait TransactionPublisher: Send + Sync + 'static {
    fn publish(&self, tx: TransactionOut) -> Box<Future<Item = (), Error = Error> + Send>;
}

#[derive(Clone)]
pub struct TransactionPublisherImpl {
    channel: Arc<Channel<TcpStream>>,
}

impl TransactionPublisherImpl {
    pub fn init(channel: Arc<Channel<TcpStream>>, users: Vec<UserId>) -> impl Future<Item = Self, Error = Error> + Send {
        let mut f = vec![];
        let f1: Box<Future<Item = (), Error = LapinError> + Send> = Box::new(channel.exchange_declare(
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
            let f2: Box<Future<Item = (), Error = LapinError> + Send> = Box::new(
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
            let f3: Box<Future<Item = (), Error = LapinError> + Send> =
                Box::new(channel.queue_bind(&queue_name, "transactions", &queue_name, Default::default(), Default::default()));
            f.push(f3);
        }
        future::join_all(f)
            .map(|_| Self { channel })
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
