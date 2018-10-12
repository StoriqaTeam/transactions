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

pub trait KeysClient: Send + Sync + 'static {
    fn create_account_address(
        &self,
        token: AuthenticationToken,
        create_account: CreateAccountAddress,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send>;
    fn create_blockchain_tx(
        &self,
        token: AuthenticationToken,
        create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct KeysClientImpl {
    cli: Arc<HttpClient>,
    keys_url: String,
}

impl KeysClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        Self {
            cli: Arc::new(cli),
            keys_url: config.client.keys_url.clone(),
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(
        &self,
        query: &str,
        body: String,
        token: AuthenticationToken,
        method: Method,
    ) -> impl Future<Item = T, Error = Error> + Send {
        let query = query.to_string();
        let query1 = query.clone();
        let query2 = query.clone();
        let query3 = query.clone();
        let cli = self.cli.clone();
        let mut builder = Request::builder();
        let url = format!("{}{}", self.keys_url, query);
        builder.uri(url).method(method);
        builder.header("Authorization", format!("Bearer {}", token.raw()));
        builder
            .body(Body::from(body))
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

impl KeysClient for KeysClientImpl {
    fn create_account_address(
        &self,
        token: AuthenticationToken,
        create_account: CreateAccountAddress,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&create_account)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_account))
                .into_future()
                .and_then(move |body| {
                    client
                        .exec_query::<CreateAccountAddressResponse>("/account_address", body, token, Method::POST)
                        .map(|resp_data| resp_data.account_address)
                }),
        )
    }
    fn create_blockchain_tx(
        &self,
        token: AuthenticationToken,
        create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&create_blockchain_tx)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_blockchain_tx))
                .into_future()
                .and_then(move |body| {
                    client
                        .exec_query::<CreateBlockchainTxResponse>("/blockchain", body, token, Method::POST)
                        .map(|resp_data| resp_data.blockchain_tx_id)
                }),
        )
    }
}

#[derive(Default)]
pub struct KeysClientMock;

impl KeysClient for KeysClientMock {
    fn create_account_address(
        &self,
        _token: AuthenticationToken,
        _create_account: CreateAccountAddress,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send> {
        Box::new(Ok(AccountAddress::default()).into_future())
    }
    fn create_blockchain_tx(
        &self,
        _token: AuthenticationToken,
        _create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send> {
        Box::new(Ok(BlockchainTransactionId::default()).into_future())
    }
}
