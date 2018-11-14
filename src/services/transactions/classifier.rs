use std::sync::Arc;

use super::super::error::*;
use models::*;
use prelude::*;
use repos::AccountsRepo;
use validator::Validate;

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
}

impl ClassifierServiceImpl {
    pub fn new(accounts_repo: Arc<AccountsRepo>) -> Self {
        Self { accounts_repo }
    }
}

impl ClassifierService for ClassifierServiceImpl {
    fn validate_and_classify_transaction(&self, input: &CreateTransactionInput) -> Result<TransactionType, Error> {
        input
            .validate()
            .map_err(|e| ectx!(try err e.clone(), ErrorKind::InvalidInput(e) => input))?;
        let from_account = self
            .accounts_repo
            .get(input.from)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => input))?;

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
