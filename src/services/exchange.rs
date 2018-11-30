use std::sync::Arc;

use super::error::*;
use client::ExchangeClient;
use models::*;
use prelude::*;

pub trait ExchangeService: Send + Sync + 'static {
    fn rate(&self, token: AuthenticationToken, input: RateInput) -> Box<Future<Item = Rate, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct ExchangeServiceImpl {
    exchange_client: Arc<ExchangeClient>,
}

impl ExchangeServiceImpl {
    pub fn new(exchange_client: Arc<ExchangeClient>) -> Self {
        Self { exchange_client }
    }
}

impl ExchangeService for ExchangeServiceImpl {
    fn rate(&self, _token: AuthenticationToken, input: RateInput) -> Box<Future<Item = Rate, Error = Error> + Send> {
        let input_clone = input.clone();
        Box::new(self.exchange_client.rate(input, Role::User).map_err(ectx!(convert => input_clone)))
    }
}
