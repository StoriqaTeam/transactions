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
        create_account: CreateAccountAddress,
        role: Role,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send>;
    fn sign_transaction(
        &self,
        create_blockchain_tx: CreateBlockchainTx,
        role: Role,
    ) -> Box<Future<Item = BlockchainTransactionRaw, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct KeysClientImpl {
    cli: Arc<HttpClient>,
    keys_url: String,
    // Todo - hack to make things quicker in upsert_system_accounts
    keys_user_id: UserId,
    keys_token: AuthenticationToken,
    keys_system_user_id: UserId,
    keys_system_token: AuthenticationToken,
    bitcoin_fee_price: Amount,
    ethereum_fee_price: Amount,
}

impl KeysClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        let bitcoin_fee_price = Amount::new(config.fee_price.bitcoin as u128);
        let ethereum_fee_price = Amount::new(config.fee_price.ethereum as u128);
        Self {
            cli: Arc::new(cli),
            keys_url: config.client.keys_url.clone(),
            keys_user_id: config.auth.keys_user_id.clone(),
            keys_token: config.auth.keys_token.clone(),
            keys_system_user_id: config.system.keys_system_user_id,
            keys_system_token: config.system.keys_system_user_token.clone(),
            bitcoin_fee_price,
            ethereum_fee_price,
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
        let url = format!("{}{}", self.keys_url, query);
        let token = match role {
            Role::System => self.keys_system_token.clone(),
            Role::User => self.keys_token.clone(),
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

impl KeysClient for KeysClientImpl {
    fn create_account_address(
        &self,
        create_account: CreateAccountAddress,
        role: Role,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send> {
        let client = self.clone();
        let user_id = match role {
            Role::System => self.keys_system_user_id,
            Role::User => self.keys_user_id,
        };
        Box::new(
            serde_json::to_string(&create_account)
                .map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => create_account))
                .into_future()
                .and_then(move |body| {
                    let url = format!("/users/{}/keys", user_id);
                    client
                        .exec_query::<CreateAccountAddressResponse>(&url, body, Method::POST, role)
                        .map(|resp_data| resp_data.blockchain_address)
                }),
        )
    }
    fn sign_transaction(
        &self,
        mut create_blockchain_tx: CreateBlockchainTx,
        role: Role,
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
                        .exec_query::<CreateBlockchainTxResponse>("/transactions", body, Method::POST, role)
                        .map(|resp_data| resp_data.raw)
                }),
        )
    }
}

#[derive(Default)]
pub struct KeysClientMock;

impl KeysClient for KeysClientMock {
    fn create_account_address(
        &self,
        _create_account: CreateAccountAddress,
        _role: Role,
    ) -> Box<Future<Item = AccountAddress, Error = Error> + Send> {
        Box::new(Ok(AccountAddress::default()).into_future())
    }
    fn sign_transaction(
        &self,
        _create_blockchain_tx: CreateBlockchainTx,
        _role: Role,
    ) -> Box<Future<Item = BlockchainTransactionRaw, Error = Error> + Send> {
        Box::new(Ok(BlockchainTransactionRaw::default()).into_future())
    }
}
