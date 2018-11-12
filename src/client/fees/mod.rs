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

pub trait FeesClient: Send + Sync + 'static {
    fn bitcoin_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send>;
    fn eth_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send>;
    fn stq_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct FeesClientImpl {
    cli: Arc<HttpClient>,
    btc_fees_collect_url: String,
    eth_fees_collect_url: String,
    btc_transaction_size: i32,
    eth_gas_limit: i32,
    stq_gas_limit: i32,
}

impl FeesClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        Self {
            cli: Arc::new(cli),
            btc_fees_collect_url: config.fees_options.btc_fees_collect_url.clone(),
            eth_fees_collect_url: config.fees_options.eth_fees_collect_url.clone(),
            btc_transaction_size: config.fees_options.btc_transaction_size,
            eth_gas_limit: config.fees_options.eth_gas_limit,
            stq_gas_limit: config.fees_options.stq_gas_limit,
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(&self, url: String, method: Method) -> impl Future<Item = T, Error = Error> + Send {
        let query = url.clone();
        let query1 = query.clone();
        let query2 = query.clone();
        let cli = self.cli.clone();
        let mut builder = Request::builder();
        builder.uri(url).method(method);
        builder.header("user-agent", "Mozilla/5.0 (X11; Ubuntu; Linu…) Gecko/20100101 Firefox/63.0");
        builder
            .body(Body::empty())
            .map_err(ectx!(ErrorSource::Hyper, ErrorKind::MalformedInput))
            .into_future()
            .and_then(move |req| cli.request(req).map_err(ectx!(ErrorKind::Internal => query1)))
            .and_then(move |resp| read_body(resp.into_body()).map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => query2)))
            .and_then(|bytes| {
                let bytes_clone = bytes.clone();
                String::from_utf8(bytes).map_err(ectx!(ErrorSource::Utf8, ErrorKind::Internal => bytes_clone))
            }).and_then(|string| serde_json::from_str::<T>(&string).map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => string)))
    }
}

impl FeesClient for FeesClientImpl {
    fn bitcoin_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        let client = self.clone();
        let url = self.btc_fees_collect_url.clone();
        let btc_transaction_size = self.btc_transaction_size;
        Box::new(
            client
                .exec_query::<BitcoinFeeResponse>(url, Method::GET)
                .map(move |resp| resp.to_fees(btc_transaction_size)),
        )
    }

    fn eth_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        let client = self.clone();
        let url = self.eth_fees_collect_url.clone();
        let eth_gas_limit = self.eth_gas_limit;
        Box::new(
            client
                .exec_query::<EthFeeResponse>(url, Method::GET)
                .map(move |resp| resp.to_fees(eth_gas_limit)),
        )
    }

    fn stq_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        let client = self.clone();
        let url = self.eth_fees_collect_url.clone();
        let stq_gas_limit = self.stq_gas_limit;
        Box::new(
            client
                .exec_query::<EthFeeResponse>(url, Method::GET)
                .map(move |resp| resp.to_fees(stq_gas_limit)),
        )
    }
}

#[derive(Default)]
pub struct FeesClientMock;

impl FeesClient for FeesClientMock {
    fn bitcoin_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        Box::new(Ok(vec![]).into_future())
    }
    fn eth_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        Box::new(Ok(vec![]).into_future())
    }
    fn stq_fees(&self) -> Box<Future<Item = Vec<Fee>, Error = Error> + Send> {
        Box::new(Ok(vec![]).into_future())
    }
}
