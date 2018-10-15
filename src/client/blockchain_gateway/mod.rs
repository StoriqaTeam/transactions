mod error;
mod requests;
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
use self::requests::*;
use self::responses::*;
use super::HttpClient;
use config::Config;
use utils::read_body;

pub trait BlockchainClient: Send + Sync + 'static {
    fn post_etherium_transaction(
        &self,
        token: AuthenticationToken,
        post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send>;
    fn post_bitcoin_transaction(
        &self,
        token: AuthenticationToken,
        post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send>;
    fn get_bitcoin_utxos(
        &self,
        token: AuthenticationToken,
        address: AccountAddress,
    ) -> Box<Future<Item = GetBitcoinUtxosResponse, Error = Error> + Send>;
    fn get_etherium_nonce(
        &self,
        token: AuthenticationToken,
        address: AccountAddress,
    ) -> Box<Future<Item = GetEtheriumNonceResponse, Error = Error> + Send>;
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
        token: &AuthenticationToken,
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
        builder.header("Authorization", format!("Bearer {}", token.raw()));
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
    fn post_etherium_transaction(
        &self,
        token: AuthenticationToken,
        post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&post_transaction)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => post_transaction))
                .into_future()
                .and_then(move |body| client.exec_query::<()>("/ethereum/transactions/raw", Some(body), &token, Method::POST)),
        )
    }
    fn post_bitcoin_transaction(
        &self,
        token: AuthenticationToken,
        post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        let client = self.clone();
        Box::new(
            serde_json::to_string(&post_transaction)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => post_transaction))
                .into_future()
                .and_then(move |body| client.exec_query::<()>("/ethereum/transactions/raw", Some(body), &token, Method::POST)),
        )
    }
    fn get_bitcoin_utxos(
        &self,
        token: AuthenticationToken,
        address: AccountAddress,
    ) -> Box<Future<Item = GetBitcoinUtxosResponse, Error = Error> + Send> {
        let url = format!("/ethereum/{}/nonce/", address);
        Box::new(self.exec_query::<GetBitcoinUtxosResponse>(&url, None, &token, Method::GET))
    }
    fn get_etherium_nonce(
        &self,
        token: AuthenticationToken,
        address: AccountAddress,
    ) -> Box<Future<Item = GetEtheriumNonceResponse, Error = Error> + Send> {
        let url = format!("/bitcoin/{}/nonce/", address);
        Box::new(self.exec_query::<GetEtheriumNonceResponse>(&url, None, &token, Method::GET))
    }
}

#[derive(Default)]
pub struct BlockchainClientMock;

impl BlockchainClient for BlockchainClientMock {
    fn post_etherium_transaction(
        &self,
        _token: AuthenticationToken,
        _post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        Box::new(Ok(()).into_future())
    }
    fn post_bitcoin_transaction(
        &self,
        _token: AuthenticationToken,
        _post_transaction: PostTransactoinRequest,
    ) -> Box<Future<Item = (), Error = Error> + Send> {
        Box::new(Ok(()).into_future())
    }
    fn get_bitcoin_utxos(
        &self,
        _token: AuthenticationToken,
        _address: AccountAddress,
    ) -> Box<Future<Item = GetBitcoinUtxosResponse, Error = Error> + Send> {
        Box::new(Ok(GetBitcoinUtxosResponse::default()).into_future())
    }
    fn get_etherium_nonce(
        &self,
        _token: AuthenticationToken,
        _address: AccountAddress,
    ) -> Box<Future<Item = GetEtheriumNonceResponse, Error = Error> + Send> {
        Box::new(Ok(GetEtheriumNonceResponse::default()).into_future())
    }
}
