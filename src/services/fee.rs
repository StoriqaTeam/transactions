use std::sync::Arc;

use futures::future::{self, Either};
use validator::{ValidationError, ValidationErrors};

use super::error::*;
use client::{ExchangeClient, FeesClient};
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, DbExecutor};

pub trait FeesService: Send + Sync + 'static {
    fn get_fees(&self, get_fees: GetFees) -> Box<Future<Item = Fees, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct FeesServiceImpl<E: DbExecutor> {
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    exchange_client: Arc<ExchangeClient>,
    fees_client: Arc<FeesClient>,
    fee_upside: f64,
}

impl<E: DbExecutor> FeesServiceImpl<E> {
    pub fn new(
        config: &Config,
        accounts_repo: Arc<dyn AccountsRepo>,
        db_executor: E,
        exchange_client: Arc<ExchangeClient>,
        fees_client: Arc<FeesClient>,
    ) -> Self {
        Self {
            accounts_repo,
            db_executor,
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

impl<E: DbExecutor> FeesService for FeesServiceImpl<E> {
    fn get_fees(&self, get_fees: GetFees) -> Box<Future<Item = Fees, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let fees_client = self.fees_client.clone();
        let currency = get_fees.currency;
        let fee_upside = self.fee_upside;
        let service = self.clone();
        let address = get_fees.account_address.clone();
        Box::new(
            db_executor
                .execute(move || {
                    accounts_repo
                        .filter_by_address(address.clone())
                        .map_err(ectx!(convert => address))
                        .and_then(|accs| {
                            if accs.len() == 0 {
                                Ok(false)
                            } else {
                                if accs.iter().all(|acc| acc.currency == currency) {
                                    Ok(true)
                                } else {
                                    let mut errors = ValidationErrors::new();
                                    let mut error = ValidationError::new("currency");
                                    error.add_param("message".into(), &"account currency differs from fee asked".to_string());
                                    error.add_param("details".into(), &"no details".to_string());
                                    errors.add("account", error);
                                    Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::InvalidInput(errors) => accs, currency))
                                }
                            }
                        })
                }).and_then(move |acc_exists| {
                    if acc_exists {
                        Either::A(future::ok(Fees::new(currency, vec![Fee::default()])))
                    } else {
                        Either::B(
                            match currency {
                                Currency::Btc => Box::new(fees_client.bitcoin_fees().map_err(ectx!(convert => currency)))
                                    as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                                Currency::Eth => Box::new(fees_client.eth_fees().map_err(ectx!(convert => currency)))
                                    as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                                Currency::Stq => Box::new(
                                    fees_client
                                        .stq_fees()
                                        .map_err(ectx!(convert => currency))
                                        .and_then(move |fees| service.convert_fees(fees, Currency::Stq, Currency::Eth)),
                                ) as Box<Future<Item = Vec<Fee>, Error = Error> + Send>,
                            }.map(move |mut fees| {
                                fees.iter_mut()
                                    .for_each(|f| f.value = Amount::new((f.value.raw() as f64 * fee_upside) as u128));
                                Fees::new(currency, fees)
                            }),
                        )
                    }
                }),
        )
    }
}
