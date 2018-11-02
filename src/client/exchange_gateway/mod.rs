mod error;

use std::sync::Arc;

use failure::Fail;
use futures::prelude::*;
use hyper::Method;
use hyper::{Body, Request};
use models::*;
use serde::Deserialize;
use serde_json;

pub use self::error::*;
use super::HttpClient;
use config::Config;
use utils::read_body;

pub trait ExchangeGatewayClient: Send + Sync + 'static {
    fn exchange(&self, exchange: ExchangeInput, role: Role) -> Box<Future<Item = Exchange, Error = Error> + Send>;
    fn rate(&self, exchange: RateInput, role: Role) -> Box<Future<Item = Rate, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct ExchangeGatewayClientImpl {
    cli: Arc<HttpClient>,
    exchange_gateway_url: String,
    exchange_gateway_user_id: UserId,
    exchange_gateway_token: AuthenticationToken,
    exchange_gateway_system_user_id: UserId,
    exchange_gateway_system_token: AuthenticationToken,
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
            exchange_gateway_system_user_id: config.system.exhange_gateway_system_user_id,
            exchange_gateway_system_token: config.system.exhange_gateway_system_user_token,
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(
        &self,
        query: &str,
        body: String,
        method: Method,
        role: Role,
    ) -> impl Future<Item = T, Error = Error> + Send {
        let query = query.to_string();
        let query1 = query.clone();
        let query2 = query.clone();
        let query3 = query.clone();
        let cli = self.cli.clone();
        let mut builder = Request::builder();
        let url = format!("{}{}", self.exchange_gateway_url, query);
        let token = match role {
            Role::System => self.exchange_gateway_system_token.clone(),
            Role::User => self.exchange_gateway_token.clone(),
        };
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

impl ExchangeGatewayClient for ExchangeGatewayClientImpl {
    fn exchange(&self, create_exchange: ExchangeInput, role: Role) -> Box<Future<Item = Exchange, Error = Error> + Send> {
        let client = self.clone();
        let user_id = self.exchange_gateway_user_id;
        Box::new(
            serde_json::to_string(&create_exchange)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_exchange))
                .into_future()
                .and_then(move |body| {
                    let url = "/exchange";
                    client.exec_query::<Exchange>(&url, body, Method::POST, role)
                }),
        )
    }

    fn rate(&self, create_rate: RateInput, role: Role) -> Box<Future<Item = Rate, Error = Error> + Send> {
        let client = self.clone();
        let user_id = self.exchange_gateway_user_id;
        Box::new(
            serde_json::to_string(&create_rate)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_rate))
                .into_future()
                .and_then(move |body| {
                    let url = "/rate";
                    client.exec_query::<Rate>(&url, body, Method::POST, role)
                }),
        )
    }
}
