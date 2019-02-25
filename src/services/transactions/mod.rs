mod blockchain;
mod classifier;
pub mod converter;

use std::collections::HashMap;
use std::sync::Arc;

use future::Either;
use futures::future;
use futures::prelude::*;
use validator::{ValidationError, ValidationErrors};

use self::blockchain::{BlockchainService, BlockchainServiceImpl, FeeEstimate};
use self::classifier::{ClassifierService, ClassifierServiceImpl, TransactionType};
use self::converter::{ConverterService, ConverterServiceImpl};
use super::auth::AuthService;
use super::error::*;
use super::system::{SystemService, SystemServiceImpl};
use client::BlockchainClient;
use client::ExchangeClient;
use client::KeysClient;
use config::Config;
use models::*;
use prelude::*;
use rabbit::TransactionPublisher;
use repos::{
    AccountsRepo, BlockchainTransactionsRepo, DbExecutor, Isolation, KeyValuesRepo, PendingBlockchainTransactionsRepo, TransactionsRepo,
};
use utils::{log_and_capture_error, log_error};

#[derive(Clone)]
pub struct TransactionsServiceImpl<E: DbExecutor> {
    config: Arc<Config>,
    auth_service: Arc<dyn AuthService>,
    blockchain_service: Arc<BlockchainService>,
    classifier_service: Arc<ClassifierService>,
    converter_service: Arc<ConverterService>,
    system_service: Arc<SystemService>,
    transactions_repo: Arc<dyn TransactionsRepo>,
    blockchain_transactions_repo: Arc<dyn BlockchainTransactionsRepo>,
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    exchange_client: Arc<dyn ExchangeClient>,
    publisher: Arc<dyn TransactionPublisher>,
}

pub trait TransactionsService: Send + Sync + 'static {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionInput,
    ) -> Box<Future<Item = TransactionOut, Error = Error> + Send>;
    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<TransactionOut>, Error = Error> + Send>;
    fn get_account_balance(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
    ) -> Box<Future<Item = AccountWithBalance, Error = Error> + Send>;
    fn get_transactions_for_user(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send>;
    fn get_account_transactions(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send>;
}

impl<E: DbExecutor> TransactionsServiceImpl<E> {
    pub fn new(
        config: Config,
        auth_service: Arc<AuthService>,
        transactions_repo: Arc<TransactionsRepo>,
        pending_transactions_repo: Arc<dyn PendingBlockchainTransactionsRepo>,
        blockchain_transactions_repo: Arc<dyn BlockchainTransactionsRepo>,
        accounts_repo: Arc<dyn AccountsRepo>,
        key_values_repo: Arc<dyn KeyValuesRepo>,
        db_executor: E,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
        exchange_client: Arc<dyn ExchangeClient>,
        publisher: Arc<dyn TransactionPublisher>,
    ) -> Self {
        let config = Arc::new(config);
        let classifier_service = Arc::new(ClassifierServiceImpl::new(
            &config,
            accounts_repo.clone(),
            transactions_repo.clone(),
        ));
        let system_service = Arc::new(SystemServiceImpl::new(accounts_repo.clone(), config.clone()));
        let blockchain_service = Arc::new(BlockchainServiceImpl::new(
            config.clone(),
            keys_client,
            blockchain_client,
            exchange_client.clone(),
            pending_transactions_repo.clone(),
            key_values_repo.clone(),
            system_service.clone(),
            db_executor.clone(),
        ));
        let converter_service = Arc::new(ConverterServiceImpl::new(
            accounts_repo.clone(),
            pending_transactions_repo.clone(),
            blockchain_transactions_repo.clone(),
            system_service.clone(),
        ));
        Self {
            config: config.clone(),
            auth_service,
            blockchain_service,
            classifier_service,
            system_service,
            transactions_repo,
            blockchain_transactions_repo,
            accounts_repo,
            db_executor,
            converter_service,
            exchange_client,
            publisher,
        }
    }

    fn create_base_tx(&self, tx: NewTransaction, dr_account: Account, cr_account: Account) -> Result<Transaction, Error> {
        let transactions_repo = self.transactions_repo.clone();
        if dr_account.currency != cr_account.currency {
            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => tx.clone(), dr_account.clone(), cr_account.clone()));
        }
        if (tx.dr_account_id != dr_account.id) || (tx.cr_account_id != cr_account.id) {
            return Err(
                ectx!(err ErrorContext::InvalidTransaction, ErrorKind::Internal => tx.clone(), dr_account.clone(), cr_account.clone()),
            );
        }
        let tx_clone = tx.clone();
        let balance = transactions_repo
            .get_accounts_balance(tx.user_id, &[dr_account])
            .map(|accounts| accounts[0].balance)
            .map_err(ectx!(try convert => tx_clone))?;
        if balance >= tx.value {
            transactions_repo.create(tx.clone()).map_err(ectx!(convert => tx.clone()))
        } else {
            let mut errors = ValidationErrors::new();
            let mut error = ValidationError::new("not_enough_balance");
            error.message = Some("account balance is not enough".into());
            errors.add("value", error);
            Err(ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::InvalidInput(serde_json::to_string(&errors).unwrap_or_default()) => tx))
        }
    }

    fn create_internal_mono_currency_tx(
        &self,
        create_tx_input: CreateTransactionInput,
        dr_account: Account,
        cr_account: Account,
    ) -> impl Future<Item = Transaction, Error = Error> + Send {
        let tx = NewTransaction {
            id: create_tx_input.id,
            gid: create_tx_input.id,
            user_id: create_tx_input.user_id,
            dr_account_id: dr_account.id,
            cr_account_id: cr_account.id,
            currency: dr_account.currency,
            value: create_tx_input.value,
            status: TransactionStatus::Done,
            blockchain_tx_id: None,
            kind: TransactionKind::Internal,
            group_kind: TransactionGroupKind::Internal,
            related_tx: None,
            meta: None,
        };
        let self_clone = self.clone();
        self.db_executor
            .execute_transaction_with_isolation(Isolation::Serializable, move || {
                self_clone.create_base_tx(tx, dr_account, cr_account)
            })
    }

    fn create_external_mono_currency_tx(
        &self,
        input: CreateTransactionInput,
        from_account: Account,
        to_blockchain_address: BlockchainAddress,
        to_currency: Currency,
        // these group params will be filled with defaults for external mono currency
        // to reuse it in external withdrawal, we put overrides here
        gid: Option<TransactionId>,
        tx_kind: Option<TransactionKind>,
        tx_group_kind: Option<TransactionGroupKind>,
        fee_currency: Option<Currency>,
        // by default the fee is written off from_account. However you can override this
        // using this param
        fee_payer_account_id: Option<AccountId>,
    ) -> impl Future<Item = Vec<Transaction>, Error = Error> + Send {
        if from_account.currency != to_currency {
            return Either::A(future::err(
                ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => from_account, to_blockchain_address, to_currency),
            ));
        };

        let gid = gid.unwrap_or(input.id);
        let value = input.value;
        let fee_currency = fee_currency.unwrap_or(from_account.currency);
        let db_executor = self.db_executor.clone();
        let db_executor_ = self.db_executor.clone();
        let transactions_repo = self.transactions_repo.clone();
        let system_service = self.system_service.clone();
        let blockchain_service = self.blockchain_service.clone();
        let self_clone = self.clone();
        let user_id_clone = input.user_id.clone();
        let from_account_clone = from_account.clone();
        let input_fee = input.fee.clone();
        Either::B(self
            .blockchain_service
            .estimate_withdrawal_fee(input.fee, fee_currency, to_currency)
            .map_err({
                let fee = input.fee.clone();
                ectx!(ErrorKind::Internal => fee, fee_currency, to_currency)
            })
            .and_then(move |FeeEstimate {gross_fee: total_fee_est,fee_price: fee_price_est,..}|{
                db_executor.execute_transaction_with_isolation(Isolation::Serializable, move || {
                    let withdrawal_accs_with_balance =
                        transactions_repo
                        .get_accounts_for_withdrawal(value, to_currency, total_fee_est)
                        .map_err(ectx!(try convert => value, to_currency, total_fee_est))?;

                    let mut total_value = Amount::new(0);
                    //double check
                    for AccountWithBalance {
                        account: acc,
                        balance: value,
                    } in &withdrawal_accs_with_balance
                    {
                        let acc_id = acc.id;
                        let balance =
                            transactions_repo
                            .get_account_balance(acc_id, AccountKind::Dr)
                            .map_err(ectx!(try convert => acc_id, AccountKind::Dr))?;
                        if balance < *value {
                            let mut errors = ValidationErrors::new();
                            let mut error = ValidationError::new("not_enough_balance");
                            error.message = Some("account balance is not enough".into());
                            errors.add("value", error);
                            return Err(
                                ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::InvalidInput(serde_json::to_string(&errors).unwrap_or_default()) => balance, value),
                            );
                        }
                        total_value = total_value
                            .checked_add(*value)
                            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => total_value, *value))?;
                    }

                    if total_value != input.value {
                        return Err(ectx!(err ErrorContext::InvalidValue, ErrorKind::Internal => input.clone(), total_value));
                    }

                    system_service
                        .get_system_fees_account(to_currency)
                        .map_err(ectx!(ErrorKind::Internal => to_currency))
                        .map(|fees_account| (fees_account, input.id, withdrawal_accs_with_balance, fee_price_est))
                })
            })
            .and_then(move |(fees_account, current_tx_id, withdrawal_accs_with_balance, fee_price_est)|{
                let new_db_transactions: Vec<(NewTransaction, Account, Account)> = Vec::new();
                futures::stream::iter_ok(withdrawal_accs_with_balance).fold((current_tx_id, new_db_transactions), move |(current_tx_id, mut acc_), AccountWithBalance {account: acc,balance: value}| {
                    let to = to_blockchain_address.clone();
                    let acc_address = acc.address.clone();
                    let gid = gid.clone();
                    let user_id_clone = user_id_clone.clone();
                    let from_account = from_account.clone();
                    let to_currency = to_currency.clone();
                    let tx_kind = tx_kind.clone();
                    let tx_group_kind = tx_group_kind.clone();
                    match to_currency {
                        x if x == Currency::Eth || x == Currency::Stq =>
                            Either::A(blockchain_service
                            .create_ethereum_tx(acc.address.clone(), to.clone(), value, fee_price_est, x)
                            .map_err(ectx!(ErrorKind::Internal => acc_address, to, value, fee_price_est, x))),
                        x if x == Currency::Btc =>
                            Either::B(blockchain_service
                            .create_bitcoin_tx(acc.address.clone(), to.clone(), value, fee_price_est)
                            .map_err(ectx!(ErrorKind::Internal => acc_address, to, value, fee_price_est))),
                        _ => unreachable!()
                    }.then(move |res| {
                        match res {
                            Ok(blockchain_tx_id_res) => {
                                let current_tx_id = current_tx_id.next();
                                let new_tx = NewTransaction {
                                    id: current_tx_id,
                                    gid,
                                    user_id: user_id_clone.clone(),
                                    dr_account_id: from_account.id.clone(),
                                    cr_account_id: acc.id.clone(),
                                    currency: to_currency,
                                    value: value,
                                    status: TransactionStatus::Pending,
                                    blockchain_tx_id: Some(blockchain_tx_id_res),
                                    kind: tx_kind.unwrap_or(TransactionKind::Withdrawal),
                                    group_kind: tx_group_kind.unwrap_or(TransactionGroupKind::Withdrawal),
                                    related_tx: None,
                                    meta: None,
                                };
                                acc_.push((new_tx, from_account.clone(), acc.clone()));
                                Ok((current_tx_id, acc_))
                            },
                            Err(e) => {
                                Err((e, acc_))
                            }
                        }
                    })
                })
                .then(move |res| {
                    match res {
                        Ok((_, new_db_transactions)) =>
                        Either::A(db_executor_.execute_transaction_with_isolation(Isolation::Serializable, move || {
                            let mut result = vec![];
                            let fee_tx = NewTransaction {
                                id: current_tx_id,
                                gid,
                                user_id: user_id_clone.clone(),
                                dr_account_id: fee_payer_account_id.unwrap_or(from_account_clone.id),
                                cr_account_id: fees_account.id,
                                currency: fee_currency,
                                value: input_fee,
                                status: TransactionStatus::Done,
                                blockchain_tx_id: None,
                                kind: TransactionKind::Fee,
                                group_kind: tx_group_kind.unwrap_or(TransactionGroupKind::Withdrawal),
                                related_tx: None,
                                meta: None,
                            };
                            // first - we are adding fee transaction
                            result.push(self_clone.create_base_tx(fee_tx, from_account_clone.clone(), fees_account.clone())?);
                            // adding all blockchain transactions
                            for (new_tx, dr, cr) in new_db_transactions {
                                result.push(self_clone.create_base_tx(new_tx, dr, cr)?);
                            }
                            Ok(result)
                        })),
                        Err((e, new_db_transactions)) => Either::B({
                            // if we have more then zero db_transactions - so we have at least one blockchain transaction sent.
                            if new_db_transactions.len() > 0 {
                                log_and_capture_error(e);
                                Either::A(db_executor_.execute_transaction_with_isolation(Isolation::Serializable, move || {

                                    let mut result = vec![];
                                    let fee_tx = NewTransaction {
                                        id: current_tx_id,
                                        gid,
                                        user_id: user_id_clone.clone(),
                                        dr_account_id: fee_payer_account_id.unwrap_or(from_account_clone.id),
                                        cr_account_id: fees_account.id,
                                        currency: fee_currency,
                                        value: input_fee,
                                        status: TransactionStatus::Done,
                                        blockchain_tx_id: None,
                                        kind: TransactionKind::Fee,
                                        group_kind: tx_group_kind.unwrap_or(TransactionGroupKind::Withdrawal),
                                        related_tx: None,
                                        meta: None,
                                    };
                                    // first - we are adding fee transaction
                                    result.push(self_clone.create_base_tx(fee_tx, from_account_clone.clone(), fees_account.clone())?);
                                    // adding all blockchain transactions successfully sent
                                    for (new_tx, dr, cr) in new_db_transactions {
                                        result.push(self_clone.create_base_tx(new_tx, dr, cr)?);
                                    }
                                    Ok(result)
                                }))
                            } else {
                                Either::B(future::err(e))
                            }
                        })
                    }
                })
            }))
    }

    fn create_internal_multi_currency_tx(
        &self,
        input: CreateTransactionInput,
        from_account: Account,
        to_account: Account,
        exchange_id: ExchangeId,
        exchange_rate: f64,
    ) -> impl Future<Item = Vec<Transaction>, Error = Error> + Send {
        let db_executor = self.db_executor.clone();
        let system_service = self.system_service.clone();
        let self_clone = self.clone();
        let exchange_input = ExchangeInput {
            id: exchange_id,
            from: from_account.currency,
            to: to_account.currency,
            rate: exchange_rate,
            actual_amount: input.value,
            amount_currency: input.value_currency,
        };
        let exchange_input_clone = exchange_input.clone();
        self.exchange_client
            .exchange(exchange_input, Role::User)
            .map_err(ectx!(convert => exchange_input_clone))
            .and_then(move |_| {
                db_executor.execute_transaction_with_isolation(Isolation::Serializable, move || {
                    let mut res: Vec<Transaction> = Vec::new();

                    let (from_value, to_value) = if from_account.currency == input.value_currency {
                        (
                            input.value,
                            input.value.convert(from_account.currency, to_account.currency, exchange_rate),
                        )
                    } else if to_account.currency == input.value_currency {
                        (
                            input.value.convert(to_account.currency, from_account.currency, 1.0 / exchange_rate),
                            input.value,
                        )
                    } else {
                        return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => input, from_account, to_account));
                    };

                    let current_tx_id = input.id;

                    // Moving money from `from` account to system liquidity account
                    let from_acct_currency = from_account.currency.clone();
                    let from_counterpart_acc = system_service
                        .get_system_liquidity_account(from_acct_currency.clone())
                        .map_err(ectx!(try ErrorKind::Internal => from_acct_currency))?;
                    let from_tx = NewTransaction {
                        id: current_tx_id,
                        gid: input.id,
                        user_id: input.user_id,
                        dr_account_id: from_account.id,
                        cr_account_id: from_counterpart_acc.id,
                        currency: from_account.currency,
                        value: from_value,
                        status: TransactionStatus::Done,
                        blockchain_tx_id: None,
                        kind: TransactionKind::MultiFrom,
                        group_kind: TransactionGroupKind::InternalMulti,
                        related_tx: None,
                        meta: None,
                    };
                    res.push(self_clone.create_base_tx(from_tx, from_account.clone(), from_counterpart_acc)?);

                    // Moving money from system liquidity account to `to` account
                    let current_tx_id = current_tx_id.next();
                    let to_acct_currency = to_account.currency.clone();
                    let to_counterpart_acc = system_service
                        .get_system_liquidity_account(to_acct_currency.clone())
                        .map_err(ectx!(try ErrorKind::Internal => to_acct_currency))?;
                    let to_tx = NewTransaction {
                        id: current_tx_id,
                        gid: input.id,
                        user_id: input.user_id,
                        dr_account_id: to_counterpart_acc.id,
                        cr_account_id: to_account.id,
                        currency: to_account.currency,
                        value: to_value,
                        status: TransactionStatus::Done,
                        blockchain_tx_id: None,
                        kind: TransactionKind::MultiTo,
                        group_kind: TransactionGroupKind::InternalMulti,
                        related_tx: None,
                        meta: None,
                    };
                    res.push(self_clone.create_base_tx(to_tx, to_counterpart_acc, to_account.clone())?);
                    Ok(res)
                })
            })
    }

    // #[allow(dead_code)]
    // fn create_external_multi_currency_tx(
    //     &self,
    //     input: CreateTransactionInput,
    //     from_account: Account,
    //     to_blockchain_address: BlockchainAddress,
    //     to_currency: Currency,
    //     exchange_id: ExchangeId,
    //     exchange_rate: f64,
    // ) -> Result<Vec<Transaction>, Error> {
    //     let transfer_account = self.system_service.get_system_transfer_account(to_currency)?;
    //     let mut res: Vec<Transaction> = Vec::new();
    //     let txs = self.create_internal_multi_currency_tx(
    //         input.clone(),
    //         from_account.clone(),
    //         transfer_account.clone(),
    //         exchange_id,
    //         exchange_rate,
    //     )?;
    //     let withdrawal_value = txs.iter().find(|tx| tx.kind == TransactionKind::MultiTo).unwrap().value;
    //     res.extend(txs.into_iter());
    //     let gid = input.id;
    //     let id = input.id.next().next(); // create_internal_multi_currency_tx took 2 ids
    //     let input = CreateTransactionInput {
    //         id,
    //         from: transfer_account.id,
    //         value: withdrawal_value,
    //         value_currency: to_currency,
    //         ..input
    //     };
    //     let txs = self.create_external_mono_currency_tx(
    //         input,
    //         transfer_account,
    //         to_blockchain_address,
    //         to_currency,
    //         Some(gid),
    //         Some(TransactionKind::Withdrawal),
    //         Some(TransactionGroupKind::WithdrawalMulti),
    //         Some(from_account.currency),
    //         Some(from_account.id),
    //     )?;
    //     res.extend(txs.into_iter());
    //     Ok(res)
    // }
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionInput,
    ) -> Box<Future<Item = TransactionOut, Error = Error> + Send> {
        let db_executor = self.db_executor.clone();
        let publisher = self.publisher.clone();
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let self_clone3 = self.clone();
        let input_clone = input.clone();
        Box::new(
            self.auth_service
                .authenticate(token.clone())
                .and_then(move |user| {
                    let input = CreateTransactionInput { user_id: user.id, ..input };
                    db_executor
                        .execute_transaction_with_isolation(Isolation::Serializable, move || {
                            self_clone.classifier_service.validate_and_classify_transaction(&input)
                        })
                        .and_then(move |tx_type| {
                            type BoxedFuture = Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
                            match tx_type.clone() {
                                TransactionType::Internal(from_account, to_account) => Box::new(
                                    self_clone3
                                        .create_internal_mono_currency_tx(input_clone, from_account, to_account)
                                        .map(|tx| vec![tx]),
                                ) as BoxedFuture,
                                TransactionType::Withdrawal(from_account, to_blockchain_address, currency) => {
                                    Box::new(self_clone3.create_external_mono_currency_tx(
                                        input_clone,
                                        from_account,
                                        to_blockchain_address,
                                        currency,
                                        None,
                                        None,
                                        None,
                                        None,
                                        None,
                                    )) as BoxedFuture
                                }
                                TransactionType::InternalExchange(from, to, exchange_id, rate) => {
                                    Box::new(self_clone3.create_internal_multi_currency_tx(input_clone, from, to, exchange_id, rate))
                                        as BoxedFuture
                                }
                                TransactionType::WithdrawalExchange(_from, _to_blockchain_address, _to_currency, _exchange_id, _rate) => {
                                    // This function is implemented but not tested. For now we disable it,
                                    // since we disable this functionality in wallet app.
                                    // self_clone.create_external_multi_currency_tx(
                                    //     input,
                                    //     from,
                                    //     to_blockchain_address,
                                    //     to_currency,
                                    //     exchange_id,
                                    //     rate,
                                    // )
                                    Box::new(future::err(ectx!(err ErrorContext::NotSupported, ErrorKind::MalformedInput))) as BoxedFuture
                                }
                            }
                            .map(|tx_group| (tx_group, tx_type))
                        })
                })
                .and_then(|(tx_group, tx_type)| {
                    // this point we already wrote transactions, incl to blockchain
                    // so if smth fails here, we need not corrupt our data
                    let db_executor = self_clone2.db_executor.clone();
                    db_executor
                        .execute_transaction_with_isolation(Isolation::RepeatableRead, move || {
                            self_clone2.converter_service.convert_transaction(tx_group)
                        })
                        .and_then(move |tx| {
                            // if transaction is internal - we need to publish it
                            // because it will never appear in blockchain
                            // so gateway will never know about it
                            info!("Checking for sending needed tx type: {:?}, tx: {:?}", tx_type, tx);
                            if let TransactionType::Internal(_, _) = tx_type {
                                let tx_out = tx.clone();
                                info!("Sending internal tx: {:?}", tx_out);
                                Either::A(
                                    publisher
                                        .publish(tx.clone())
                                        .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal => tx_out))
                                        .then(|r: Result<(), Error>| match r {
                                            Err(e) => {
                                                log_error(&e);
                                                Ok(tx)
                                            }
                                            Ok(_) => Ok(tx),
                                        }),
                                )
                            } else {
                                Either::B(future::ok(tx))
                            }
                        })
                }),
        )
    }

    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<TransactionOut>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let transaction = transactions_repo
                    .get(transaction_id)
                    .map_err(ectx!(try convert => transaction_id))?;
                if let Some(ref transaction) = transaction {
                    if transaction.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    let tx_group = transactions_repo
                        .get_by_gid(transaction.gid)
                        .map_err(ectx!(try convert => transaction_id))?;
                    let tx_out = self_clone.converter_service.convert_transaction(tx_group)?;
                    return Ok(Some(tx_out));
                }
                Ok(None)
            })
        }))
    }
    fn get_account_balance(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
    ) -> Box<Future<Item = AccountWithBalance, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || -> Result<AccountWithBalance, Error> {
                let account = accounts_repo.get(account_id).map_err(ectx!(try convert => account_id))?;
                if let Some(account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    transactions_repo
                        .get_accounts_balance(user.id, &[account])
                        .map(|accounts| accounts[0].clone())
                        .map_err(ectx!(convert => account_id))
                } else {
                    return Err(ectx!(err ErrorContext::NoAccount, ErrorKind::NotFound => account_id));
                }
            })
        }))
    }
    fn get_transactions_for_user(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || -> Result<Vec<TransactionOut>, Error> {
                if user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                let txs = transactions_repo
                    .list_groups_for_user_skip_approval(user_id, offset, limit)
                    .map_err(ectx!(try convert => user_id, offset, limit))?;
                let res: Result<Vec<TransactionOut>, Error> = group_transactions(&txs)
                    .into_iter()
                    .map(|tx_group| self_clone.converter_service.convert_transaction(tx_group))
                    .collect();
                let mut res = res?;
                res.sort_by_key(|tx| tx.created_at);
                let res: Vec<_> = res.into_iter().rev().collect();
                Ok(res)
            })
        }))
    }
    fn get_account_transactions(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
        offset: i64,
        limit: i64,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let account = accounts_repo.get(account_id).map_err(ectx!(try convert => account_id))?;
                if let Some(ref account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                } else {
                    return Err(ectx!(err ErrorContext::NoAccount, ErrorKind::NotFound => account_id));
                }
                let txs = transactions_repo
                    .list_groups_for_account_skip_approval(account_id, offset, limit)
                    .map_err(ectx!(try convert => account_id))?;
                let res: Result<Vec<TransactionOut>, Error> = group_transactions(&txs)
                    .into_iter()
                    .map(|tx_group| self_clone.converter_service.convert_transaction(tx_group))
                    .collect();
                let mut res = res?;
                res.sort_by_key(|tx| tx.created_at);
                let res: Vec<_> = res.into_iter().rev().collect();
                Ok(res)
            })
        }))
    }
}

// group transactions into subgroups of related txs. I.e. group tx itself + fee
fn group_transactions(transactions: &[Transaction]) -> Vec<Vec<Transaction>> {
    let mut res: HashMap<TransactionId, Vec<Transaction>> = HashMap::new();
    for tx in transactions.into_iter() {
        res.entry(tx.gid).and_modify(|txs| txs.push(tx.clone())).or_insert(vec![tx.clone()]);
    }
    res.into_iter().map(|(_, txs)| txs).collect()
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    use super::*;
    use client::*;
    use config::Config;
    use rabbit::*;
    use repos::*;
    use services::*;
    use tokio_core::reactor::Core;

    fn create_transaction_service(token: AuthenticationToken, user_id: UserId) -> TransactionsServiceImpl<DbExecutorMock> {
        let config = Config::new().unwrap();
        let auth_service = Arc::new(AuthServiceMock::new(vec![(token, user_id)]));
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let transactions_repo = Arc::new(TransactionsRepoMock::default());
        let pending_transactions_repo = Arc::new(PendingBlockchainTransactionsRepoMock::default());
        let blockchain_transactions_repo = Arc::new(BlockchainTransactionsRepoMock::default());
        let key_values_repo = Arc::new(KeyValuesRepoMock::default());
        let keys_client = Arc::new(KeysClientMock::default());
        let blockchain_client = Arc::new(BlockchainClientMock::default());
        let exchange_client = Arc::new(ExchangeClientMock::default());
        let db_executor = DbExecutorMock::default();
        let publisher = Arc::new(TransactionPublisherMock::default());
        TransactionsServiceImpl::new(
            config,
            auth_service,
            transactions_repo,
            pending_transactions_repo,
            blockchain_transactions_repo,
            accounts_repo,
            key_values_repo,
            db_executor,
            keys_client,
            blockchain_client,
            exchange_client,
            publisher,
        )
    }
}
