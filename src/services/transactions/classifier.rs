use std::sync::Arc;

use chrono::Duration;
use serde_json;
use validator::{Validate, ValidationError, ValidationErrors};

use super::super::error::*;
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, TransactionsRepo};

#[derive(Debug, Clone, PartialEq)]
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

    fn check_account_daily_limit(&self, input: &CreateTransactionInput, account: &Account) -> Result<(), Error> {
        let spending = self
            .transactions_repo
            .get_account_spending(account.id, account.kind, self.limit_period)?;
        let from_currency = account.currency;
        let to_currency = input.to_currency;
        let from_value = match input.value_currency {
            currency if currency == from_currency => input.value,
            currency if currency == to_currency => {
                if let Some(rate) = input.exchange_rate {
                    // we trust user input here, since o/w the exchange will fail anyway
                    input.value.convert(to_currency, from_currency, 1.0 / rate)
                } else {
                    return Err(ectx!(err ErrorContext::MissingExchangeRate, ErrorKind::MalformedInput));
                }
            }
            _ => return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::MalformedInput)),
        };
        let spending = spending
            .checked_add(from_value)
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        let limit = match account.currency {
            Currency::Btc => self.btc_satoshi_limit,
            Currency::Eth => self.eth_wei_limit,
            Currency::Stq => self.stq_wei_limit,
        };
        if spending > limit {
            let mut errors = ValidationErrors::new();
            let mut error = ValidationError::new("exceeded_daily_limit");
            error.message = Some("daily limit for the account exceeded".into());
            error.add_param("limit".into(), &limit.to_super_unit(account.currency).to_string());
            error.add_param("currency".into(), &account.currency.to_string().to_uppercase());
            errors.add("value", error);
            return Err(
                ectx!(err ErrorContext::LimitExceeded, ErrorKind::InvalidInput(serde_json::to_string(&errors).unwrap_or_default()) => spending, limit),
            );
        }
        Ok(())
    }

    fn get_from_account(&self, input: &CreateTransactionInput) -> Result<Account, Error> {
        self.accounts_repo
            .get(input.from)?
            .ok_or(ectx!(err ErrorContext::NoAccount, ErrorKind::NotFound => input))
    }

    fn get_to_account(&self, input: &CreateTransactionInput) -> Result<Option<Account>, Error> {
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
                Ok(Some(to_account))
            }
            RecepientType::Address => {
                let to_address = input.to.clone().to_account_address();
                self.accounts_repo
                    .get_by_address(to_address.clone(), input.to_currency, AccountKind::Cr)
                    .map_err(From::from)
            }
        }
    }

    fn get_transaction_type(
        &self,
        input: &CreateTransactionInput,
        from_account: Account,
        to_account: Option<Account>,
    ) -> Result<TransactionType, Error> {
        match to_account {
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
            None => {
                // check that we don't own any other accounts with this address
                // eg a user accidentially put ether address to receive stq tokens
                let to_address = input.to.clone().to_account_address();
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
        }
    }
}

impl ClassifierService for ClassifierServiceImpl {
    fn validate_and_classify_transaction(&self, input: &CreateTransactionInput) -> Result<TransactionType, Error> {
        input
            .validate()
            .map_err(|e| ectx!(try err e.clone(), ErrorKind::InvalidInput(serde_json::to_string(&e).unwrap_or_default()) => input))?;
        let from_account = self.get_from_account(input)?;
        self.check_account_daily_limit(input, &from_account)?;
        let to_account = self.get_to_account(input)?;
        self.get_transaction_type(input, from_account, to_account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Config;
    use repos::*;

    fn create_classifier_service(accounts_repo: Arc<dyn AccountsRepo>) -> ClassifierServiceImpl {
        let config = Config::new().unwrap();
        let transactions_repo = Arc::new(TransactionsRepoMock::default());
        ClassifierServiceImpl::new(&config, accounts_repo, transactions_repo)
    }

    fn create_internal_transaction_input(
        user_id: UserId,
        from: AccountId,
        from_currency: Currency,
        to: Recepient,
        to_type: RecepientType,
        to_currency: Currency,
        value: Amount,
    ) -> CreateTransactionInput {
        CreateTransactionInput {
            id: TransactionId::generate(),
            user_id,
            from,
            to,
            to_type,
            to_currency,
            value,
            value_currency: from_currency,
            fee: Amount::default(),
            exchange_id: None,
            exchange_rate: None,
        }
    }

    fn create_internal_exchange_transaction_input(
        user_id: UserId,
        from: AccountId,
        from_currency: Currency,
        to: Recepient,
        to_type: RecepientType,
        to_currency: Currency,
        value: Amount,
        exchange_id: Option<ExchangeId>,
        exchange_rate: Option<f64>,
    ) -> CreateTransactionInput {
        CreateTransactionInput {
            id: TransactionId::generate(),
            user_id,
            from,
            to,
            to_type,
            to_currency,
            value,
            value_currency: from_currency,
            fee: Amount::default(),
            exchange_id,
            exchange_rate,
        }
    }

    #[test]
    fn test_classify_internal_happy() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(res, TransactionType::Internal(acc1.clone(), acc2.clone()));

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(res, TransactionType::Internal(acc1, acc2));
    }

    #[test]
    fn test_classify_internal_one_account() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc1.id.to_string()),
            RecepientType::Account,
            acc1.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(res, TransactionType::Internal(acc1.clone(), acc1.clone()));

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc1.address.to_string()),
            RecepientType::Address,
            acc1.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(res, TransactionType::Internal(acc1.clone(), acc1));
    }

    #[test]
    fn test_classify_internal_exceed_limit() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc1.id.to_string()),
            RecepientType::Account,
            acc1.currency,
            Amount::new(9999999999999999999999999),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc1.address.to_string()),
            RecepientType::Address,
            acc1.currency,
            Amount::new(99999999999999999999999999),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
    }

    #[test]
    fn test_classify_internal_wrong_currencies() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Stq;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Stq;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            Currency::Eth,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            Currency::Eth,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            Currency::Eth,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            Currency::Eth,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
    }

    #[test]
    fn test_classify_internal_wrong_account_ids() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let input = create_internal_transaction_input(
            user_id,
            AccountId::generate(),
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            AccountId::generate(),
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(AccountId::generate().to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
    }

    #[test]
    fn test_classify_internal_exchange_happy() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Btc;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Stq;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let exchange_id = Some(ExchangeId::generate());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(
            res,
            TransactionType::InternalExchange(acc1.clone(), acc2.clone(), exchange_id.unwrap(), 1f64)
        );

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input).unwrap();
        assert_eq!(
            res,
            TransactionType::InternalExchange(acc1.clone(), acc2.clone(), exchange_id.unwrap(), 1f64)
        );
    }

    #[test]
    fn test_classify_internal_exchange_wrong_exchange_data() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Btc;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Stq;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let exchange_id = Some(ExchangeId::generate());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(0f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(0f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
            None,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
            None,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            None,
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            None,
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
    }

    #[test]
    fn test_classify_internal_exchange_wrong_currencies() {
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let user_id = UserId::generate();
        let service = create_classifier_service(accounts_repo.clone());
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Btc;
        let acc1 = accounts_repo.create(new_account.clone()).unwrap();
        let mut new_account = NewAccount::default();
        new_account.user_id = user_id;
        new_account.currency = Currency::Eth;
        let acc2 = accounts_repo.create(new_account).unwrap();

        let exchange_id = Some(ExchangeId::generate());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            Currency::Stq,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            Currency::Stq,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            acc2.currency,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.id.to_string()),
            RecepientType::Account,
            Currency::Stq,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());

        let input = create_internal_exchange_transaction_input(
            user_id,
            acc1.id,
            acc1.currency,
            Recepient::new(acc2.address.to_string()),
            RecepientType::Address,
            Currency::Stq,
            Amount::new(0),
            exchange_id,
            Some(1f64),
        );

        let res = service.validate_and_classify_transaction(&input);
        assert!(res.is_err());
    }
}
