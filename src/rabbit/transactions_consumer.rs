use futures::future;
use lapin_futures::channel::{BasicConsumeOptions, Channel, QueueDeclareOptions};
use lapin_futures::consumer::Consumer;
use lapin_futures::types::FieldTable;
use tokio::net::tcp::TcpStream;

use super::error::*;
use super::r2d2::RabbitConnectionManager;
use models::*;
use prelude::*;

#[derive(Clone)]
pub struct TransactionConsumerImpl {
    rabbit_pool: RabbitConnectionManager,
}

impl TransactionConsumerImpl {
    pub fn new(rabbit_pool: RabbitConnectionManager) -> Self {
        Self { rabbit_pool }
    }

    pub fn subscribe(&self) -> impl Future<Item = Vec<(Consumer<TcpStream>, Channel<TcpStream>)>, Error = Error> {
        let self_clone = self.clone();
        let fs = vec![Currency::Btc, Currency::Eth, Currency::Stq].into_iter().map(move |currency| {
            let self_clone2 = self_clone.clone();
            self_clone
                .get_channel()
                .and_then(move |channel| self_clone2.subscribe_for_currency(&channel, currency))
        });
        future::join_all(fs)
    }

    fn get_channel(&self) -> impl Future<Item = Channel<TcpStream>, Error = Error> {
        self.rabbit_pool
            .get_channel()
            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
            .into_future()
    }

    fn subscribe_for_currency(
        &self,
        channel: &Channel<TcpStream>,
        currency: Currency,
    ) -> impl Future<Item = (Consumer<TcpStream>, Channel<TcpStream>), Error = Error> {
        let queue_name = format!("{}_transactions", currency);
        let channel_clone = channel.clone();
        channel
            .queue_declare(
                &queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                Default::default(),
            )
            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
            .and_then(move |queue| {
                channel_clone
                    .basic_consume(&queue, "", BasicConsumeOptions::default(), FieldTable::new())
                    .map(move |consumer| (consumer, channel_clone))
                    .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
            })
    }
}
