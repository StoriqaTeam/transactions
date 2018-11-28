use std::sync::Arc;

use chrono::Duration;
use serde_json;
use validator::{Validate, ValidationError, ValidationErrors};

use super::super::error::*;
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, TransactionsRepo};

#[derive(Debug, Clone)]
pub enum TransactionType {
    Internal(Account, Account),
    Withdrawal(Account, BlockchainAddress, Currency),
    InternalExchange(Account, Account, ExchangeId, f64),
    WithdrawalExchange(Account, BlockchainAddress, Currency, ExchangeId, f64),
}

pub trait ClassifierService: Send + Sync + 'static {
    fn validate_and_classify_transaction(&self, input: &CreateTransactionInput) -> Result<TransactionType, Error>;
}

#[derive(Clone)]
pub struct ClassifierServiceImpl {
    accounts_repo: Arc<AccountsRepo>,
    transactions_repo: Arc<TransactionsRepo>,
    stq_wei_limit: Amount,
    eth_wei_limit: Amount,
    btc_satoshi_limit: Amount,
    limit_period: Duration,
}

const WEI_IN_ETH: u128 = 1_000_000_000_000_000_000;
const SATOSHI_IN_BTC: u128 = 100_000_000;

impl ClassifierServiceImpl {
    pub fn new(config: &Config, accounts_repo: Arc<AccountsRepo>, transactions_repo: Arc<TransactionsRepo>) -> Self {
        let stq_wei_limit = Amount::new((config.limits.stq_limit as u128) * WEI_IN_ETH);
        let eth_wei_limit = Amount::new(((config.limits.eth_limit * 1000.0) as u128) * WEI_IN_ETH / 1000);
        let btc_satoshi_limit = Amount::new(((config.limits.btc_limit * 1000.0) as u128) * SATOSHI_IN_BTC / 1000);
        let limit_period = Duration::seconds(config.limits.period_secs as i64);
        Self {
            accounts_repo,
            transactions_repo,
            stq_wei_limit,
            eth_wei_limit,
            btc_satoshi_limit,
            limit_period,
        }
    }
}

impl ClassifierService for ClassifierServiceImpl {
    fn validate_and_classify_transaction(&self, input: &CreateTransactionInput) -> Result<TransactionType, Error> {
        input
            .validate()
            .map_err(|e| ectx!(try err e.clone(), ErrorKind::InvalidInput(serde_json::to_string(&e).unwrap_or_default()) => input))?;
        let from_account = self
            .accounts_repo
            .get(input.from)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => input))?;
        let spending = self
            .transactions_repo
            .get_account_spending(from_account.id, from_account.kind, self.limit_period)?;
        let from_value = if input.value_currency == from_account.currency {
            input.value
        } else if let Some(rate) = input.exchange_rate {
            // we trust user input here, since o/w the exchange will fail anyway
            input.value.convert(input.value_currency, from_account.currency, 1.0 / rate)
        } else {
            return Err(ectx!(err ErrorContext::MissingExchangeRate, ErrorKind::MalformedInput));
        };

        let spending = spending
            .checked_add(from_value)
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let limit = match from_account.currency {
            Currency::Btc => self.btc_satoshi_limit,
            Currency::Eth => self.eth_wei_limit,
            Currency::Stq => self.stq_wei_limit,
        };
        if spending > limit {
            let mut errors = ValidationErrors::new();
            let mut error = ValidationError::new("exceeded_daily_limit");
            error.message = Some("daily limit for the account exceeded".into());
            error.add_param("limit".into(), &limit.to_super_unit(from_account.currency).to_string());
            errors.add("value", error);
            return Err(
                ectx!(err ErrorContext::LimitExceeded, ErrorKind::InvalidInput(serde_json::to_string(&errors).unwrap_or_default()) => spending, limit),
            );
        }
        match input.to_type {
            RecepientType::Account => {
                let to_account_id = input
                    .to
                    .clone()
                    .to_account_id()
                    .map_err(|_| ectx!(try err ErrorContext::InvalidUuid, ErrorKind::MalformedInput => input.clone()))?;
                let to_account = self
                    .accounts_repo
                    .get(to_account_id)?
                    .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => input))?;
                if to_account.currency != input.to_currency {
                    return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::MalformedInput => input));
                }
                if from_account.currency != to_account.currency {
                    let (exchange_id, exchange_rate) = match (input.exchange_id, input.exchange_rate) {
                        (Some(exchange_id), Some(exchange_rate)) => (exchange_id, exchange_rate),
                        _ => return Err(ectx!(err ErrorContext::MissingExchangeRate, ErrorKind::MalformedInput => input)),
                    };
                    if (input.value_currency != from_account.currency) && (input.value_currency != to_account.currency) {
                        return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::MalformedInput => input));
                    }
                    Ok(TransactionType::InternalExchange(
                        from_account,
                        to_account,
                        exchange_id,
                        exchange_rate,
                    ))
                } else {
                    Ok(TransactionType::Internal(from_account, to_account))
                }
            }
            RecepientType::Address => {
                let to_address = input.to.clone().to_account_address();
                match self
                    .accounts_repo
                    .get_by_address(to_address.clone(), input.to_currency, AccountKind::Cr)?
                {
                    None => {
                        // check that we don't own any other accounts with this address
                        // eg a user accidentally put ehter address to recieve stq tokens
                        let accounts = self.accounts_repo.filter_by_address(to_address.clone())?;
                        if accounts.len() != 0 {
                            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::MalformedInput => input.clone()));
                        }
                        if from_account.currency != input.to_currency {
                            let (exchange_id, exchange_rate) = match (input.exchange_id, input.exchange_rate) {
                                (Some(exchange_id), Some(exchange_rate)) => (exchange_id, exchange_rate),
                                _ => return Err(ectx!(err ErrorContext::MissingExchangeRate, ErrorKind::MalformedInput => input)),
                            };

                            Ok(TransactionType::WithdrawalExchange(
                                from_account,
                                to_address,
                                input.to_currency,
                                exchange_id,
                                exchange_rate,
                            ))
                        } else {
                            Ok(TransactionType::Withdrawal(from_account, to_address, input.to_currency))
                        }
                    }
                    Some(to_account) => {
                        if from_account.currency != to_account.currency {
                            let (exchange_id, exchange_rate) = match (input.exchange_id, input.exchange_rate) {
                                (Some(exchange_id), Some(exchange_rate)) => (exchange_id, exchange_rate),
                                _ => return Err(ectx!(err ErrorContext::MissingExchangeRate, ErrorKind::MalformedInput => input)),
                            };
                            Ok(TransactionType::InternalExchange(
                                from_account,
                                to_account,
                                exchange_id,
                                exchange_rate,
                            ))
                        } else {
                            Ok(TransactionType::Internal(from_account, to_account))
                        }
                    }
                }
            }
        }
    }
}
