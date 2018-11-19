use std::sync::Arc;

use super::error::*;
use client::{ExchangeClient, FeesClient};
use config::Config;
use models::*;
use prelude::*;

pub trait FeesService: Send + Sync + 'static {
    fn get_fees(&self, get_fees: GetFees) -> Box<Future<Item = Fees, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct FeesServiceImpl {
    exchange_client: Arc<ExchangeClient>,
    fees_client: Arc<FeesClient>,
    fee_upside: f64,
}

impl FeesServiceImpl {
    pub fn new(config: &Config, exchange_client: Arc<ExchangeClient>, fees_client: Arc<FeesClient>) -> Self {
        Self {
            exchange_client,
            fees_client,
            fee_upside: config.fees_options.fee_upside,
        }
    }

    pub fn convert_fees(&self, mut fees: Vec<Fee>, from: Currency, to: Currency) -> impl Future<Item = Vec<Fee>, Error = Error> + Send {
        let amount = fees.iter().map(|f| f.value).nth(0).unwrap_or_default();
        let rate_input = RateInput::new(from, to, amount, to);
        let rate_input_clone = rate_input.clone();
        let exchange_client = self.exchange_client.clone();
        exchange_client
            .rate(rate_input, Role::System)
            .map_err(ectx!(convert => rate_input_clone))
            .map(|rate_resp| {
                let rate = rate_resp.rate;
                fees.iter_mut()
                    .for_each(|f| f.value = Amount::new((f.value.raw() as f64 / rate) as u128));
                fees
            })
    }
}

impl FeesService for FeesServiceImpl {
    fn get_fees(&self, get_fees: GetFees) -> Box<Future<Item = Fees, Error = Error> + Send> {
        let fees_client = self.fees_client.clone();
        let to_currency = get_fees.to_currency;
        let from_currency = get_fees.from_currency;
        let fee_upside = self.fee_upside;
        let service = self.clone();
        Box::new(
            match (from_currency, to_currency) {
                (Currency::Btc, Currency::Btc) => {
                    Box::new(fees_client.bitcoin_fees().map_err(ectx!(convert => from_currency, to_currency)))
                        as Box<Future<Item = Vec<Fee>, Error = Error> + Send>
                }
                (Currency::Btc, Currency::Eth) => Box::new(
                    fees_client
                        .eth_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Btc, Currency::Eth)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Btc, Currency::Stq) => Box::new(
                    fees_client
                        .stq_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Btc, Currency::Eth)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Eth, Currency::Eth) => Box::new(fees_client.eth_fees().map_err(ectx!(convert => from_currency, to_currency)))
                    as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Eth, Currency::Btc) => Box::new(
                    fees_client
                        .bitcoin_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Eth, Currency::Btc)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Eth, Currency::Stq) => Box::new(fees_client.stq_fees().map_err(ectx!(convert => from_currency, to_currency)))
                    as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Stq, Currency::Stq) => Box::new(
                    fees_client
                        .stq_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Stq, Currency::Eth)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Stq, Currency::Btc) => Box::new(
                    fees_client
                        .bitcoin_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Stq, Currency::Btc)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                (Currency::Stq, Currency::Eth) => Box::new(
                    fees_client
                        .eth_fees()
                        .map_err(ectx!(convert => from_currency, to_currency))
                        .and_then(move |fees| service.convert_fees(fees, Currency::Stq, Currency::Eth)),
                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
            }.map(move |mut fees| {
                fees.iter_mut()
                    .for_each(|f| f.value = Amount::new((f.value.raw() as f64 * fee_upside) as u128));
                Fees::new(from_currency, fees)
            }),
        )
    }
}
