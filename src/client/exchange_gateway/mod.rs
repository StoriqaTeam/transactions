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

pub trait ExchangeGatewayClient: Send + Sync + 'static {
    fn exchange(&self, exchange: ExchangeInput) -> Box<Future<Item = Exchange, Error = Error> + Send>;
    fn sign_transaction(
        &self,
        create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionRaw, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct ExchangeGatewayClientImpl {
    cli: Arc<HttpClient>,
    exchange_gateway_url: String,
    // Todo - hack to make things quicker in upsert_system_accounts
    exchange_gateway_user_id: UserId,
    exchange_gateway_token: AuthenticationToken,
    bitcoin_fee_price: Amount,
    ethereum_fee_price: Amount,
}

impl ExchangeGatewayClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        let bitcoin_fee_price = Amount::new(config.fee_price.bitcoin as u128);
        let ethereum_fee_price = Amount::new(config.fee_price.ethereum as u128);
        Self {
            cli: Arc::new(cli),
            exchange_gateway_url: config.client.exchange_gateway_url.clone(),
            exchange_gateway_user_id: config.auth.exchange_gateway_user_id.clone(),
            exchange_gateway_token: config.auth.exchange_gateway_token.clone(),
            bitcoin_fee_price,
            ethereum_fee_price,
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(
        &self,
        query: &str,
        body: String,
        method: Method,
    ) -> impl Future<Item = T, Error = Error> + Send {
        let query = query.to_string();
        let query1 = query.clone();
        let query2 = query.clone();
        let query3 = query.clone();
        let cli = self.cli.clone();
        let mut builder = Request::builder();
        let url = format!("{}{}", self.exchange_gateway_url, query);
        builder.uri(url).method(method);
        builder.header("Authorization", format!("Bearer {}", self.exchange_gateway_token.raw()));
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

impl ExchangeGatewayClient for ExchangeGatewayClientImpl {
    fn exchange(&self, create_exchange: ExchangeInput) -> Box<Future<Item = Exchange, Error = Error> + Send> {
        let client = self.clone();
        let user_id = self.exchange_gateway_user_id;
        Box::new(
            serde_json::to_string(&create_account)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_account))
                .into_future()
                .and_then(move |body| {
                    let url = format!("/users/{}/exchange_gateway", user_id);
                    client
                        .exec_query::<CreateAccountAddressResponse>(&url, body, Method::POST)
                        .map(|resp_data| resp_data.blockchain_address)
                }),
        )
    }
    fn sign_transaction(
        &self,
        mut create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionRaw, Error = Error> + Send> {
        let client = self.clone();
        create_blockchain_tx.fee_price = match create_blockchain_tx.currency {
            Currency::Btc => self.bitcoin_fee_price,
            Currency::Eth | Currency::Stq => self.ethereum_fee_price,
        };
        Box::new(
            serde_json::to_string(&create_blockchain_tx)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_blockchain_tx))
                .into_future()
                .and_then(move |body| {
                    client
                        .exec_query::<CreateBlockchainTxResponse>("/transactions", body, Method::POST)
                        .map(|resp_data| resp_data.raw)
                }),
        )
    }
}

#[derive(Default)]
pub struct ExchangeGatewayClientMock;

impl ExchangeGatewayClient for ExchangeGatewayClientMock {
    fn create_account_address(&self, _create_account: CreateAccountAddress) -> Box<Future<Item = AccountAddress, Error = Error> + Send> {
        Box::new(Ok(AccountAddress::default()).into_future())
    }
    fn sign_transaction(
        &self,
        _create_blockchain_tx: CreateBlockchainTx,
    ) -> Box<Future<Item = BlockchainTransactionRaw, Error = Error> + Send> {
        Box::new(Ok(BlockchainTransactionRaw::default()).into_future())
    }
}
