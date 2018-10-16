mod error;
mod responses;

use std::sync::Arc;

use failure::Fail;
use futures::prelude::*;
use hyper::Method;
use hyper::{Body, Request};
use models::*;
use serde::Deserialize;
use serde_json;

pub use self::error::*;
use self::responses::*;
use super::HttpClient;
use config::Config;
use utils::read_body;

pub trait BlockchainClient: Send + Sync + 'static {
    fn post_ethereum_transaction(
        &self,
        transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send>;
    fn post_bitcoin_transaction(
        &self,
        transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send>;
    fn get_bitcoin_utxos(&self, address: AccountAddress) -> Box<Future<Item = Vec<BitcoinUtxos>, Error = Error> + Send>;
    fn get_ethereum_nonce(&self, address: AccountAddress) -> Box<Future<Item = u64, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct BlockchainClientImpl {
    cli: Arc<HttpClient>,
    blockchain_url: String,
}

impl BlockchainClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        Self {
            cli: Arc::new(cli),
            blockchain_url: config.client.blockchain_url.clone(),
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(
        &self,
        query: &str,
        body: Option<String>,
        method: Method,
    ) -> impl Future<Item = T, Error = Error> + Send {
        let query = query.to_string();
        let query1 = query.clone();
        let query2 = query.clone();
        let query3 = query.clone();
        let cli = self.cli.clone();
        let mut builder = Request::builder();
        let url = format!("{}{}", self.blockchain_url, query);
        builder.uri(url).method(method);
        let body = if let Some(body) = body { Body::from(body) } else { Body::empty() };
        builder
            .body(body)
            .map_err(ectx!(ErrorSource::Hyper, ErrorKind::MalformedInput => query3))
            .into_future()
            .and_then(move |req| cli.request(req).map_err(ectx!(ErrorKind::Internal => query1)))
            .and_then(move |resp| read_body(resp.into_body()).map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => query2)))
            .and_then(|bytes| {
                let bytes_clone = bytes.clone();
                String::from_utf8(bytes).map_err(ectx!(ErrorSource::Utf8, ErrorKind::Internal => bytes_clone))
            }).and_then(|string| serde_json::from_str::<T>(&string).map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => string)))
    }
}

impl BlockchainClient for BlockchainClientImpl {
    fn post_ethereum_transaction(
        &self,
        transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&transaction)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => transaction))
                .into_future()
                .and_then(move |body| client.exec_query::<BlockchainTransactionId>("/ethereum/transactions/raw", Some(body), Method::POST)),
        )
    }
    fn post_bitcoin_transaction(
        &self,
        transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&transaction)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => transaction))
                .into_future()
                .and_then(move |body| client.exec_query::<BlockchainTransactionId>("/ethereum/transactions/raw", Some(body), Method::POST)),
        )
    }
    fn get_bitcoin_utxos(&self, address: AccountAddress) -> Box<Future<Item = Vec<BitcoinUtxos>, Error = Error> + Send> {
        let url = format!("/ethereum/{}/nonce/", address);
        Box::new(
            self.exec_query::<GetBitcoinUtxosResponse>(&url, None, Method::GET)
                .map(|resp| resp.utxos),
        )
    }
    fn get_ethereum_nonce(&self, address: AccountAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
        let url = format!("/bitcoin/{}/nonce/", address);
        Box::new(
            self.exec_query::<GetEtheriumNonceResponse>(&url, None, Method::GET)
                .map(|resp| resp.nonce),
        )
    }
}

#[derive(Default)]
pub struct BlockchainClientMock;

impl BlockchainClient for BlockchainClientMock {
    fn post_ethereum_transaction(
        &self,
        _post_transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        Box::new(Ok(BlockchainTransactionId::default()).into_future())
    }
    fn post_bitcoin_transaction(
        &self,
        _post_transaction: BlockchainTransaction,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        Box::new(Ok(BlockchainTransactionId::default()).into_future())
    }
    fn get_bitcoin_utxos(&self, _address: AccountAddress) -> Box<Future<Item = Vec<BitcoinUtxos>, Error = Error> + Send> {
        Box::new(Ok(vec![BitcoinUtxos::default()]).into_future())
    }
    fn get_ethereum_nonce(&self, _address: AccountAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
        Box::new(Ok(0).into_future())
    }
}
