mod blockchain;
mod classifier;
mod converter;
mod system;

use std::collections::HashMap;
use std::sync::Arc;

use futures::future;
use futures::prelude::*;

use self::blockchain::{BlockchainService, BlockchainServiceImpl, FeeEstimate};
use self::classifier::{ClassifierService, ClassifierServiceImpl, TransactionType};
use self::converter::{ConverterService, ConverterServiceImpl};
use self::system::{SystemService, SystemServiceImpl};
use super::auth::AuthService;
use super::error::*;
use client::BlockchainClient;
use client::ExchangeClient;
use client::KeysClient;
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, BlockchainTransactionsRepo, DbExecutor, Isolation, PendingBlockchainTransactionsRepo, TransactionsRepo};
use tokio_core::reactor::Core;
use utils::log_and_capture_error;

const MAX_TRANSACTIONS_PER_TRANSACTION_OUT: i64 = 3;

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
        db_executor: E,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
        exchange_client: Arc<dyn ExchangeClient>,
    ) -> Self {
        let config = Arc::new(config);
        let blockchain_service = Arc::new(BlockchainServiceImpl::new(
            config.clone(),
            keys_client,
            blockchain_client,
            exchange_client.clone(),
            pending_transactions_repo,
        ));
        let classifier_service = Arc::new(ClassifierServiceImpl::new(accounts_repo.clone()));
        let system_service = Arc::new(SystemServiceImpl::new(accounts_repo.clone(), config));
        let converter_service = Arc::new(ConverterServiceImpl::new(
            accounts_repo.clone(),
            pending_transactions_repo.clone(),
            blockchain_transactions_repo.clone(),
            system_service,
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
        }
    }

    fn create_base_tx(&self, tx: NewTransaction, dr_account: Account, cr_account: Account) -> Result<Transaction, Error> {
        if dr_account.currency != cr_account.currency {
            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => tx.clone(), dr_account.clone(), cr_account.clone()));
        }
        if (tx.dr_account_id != dr_account.id) || (tx.cr_account_id != cr_account.id) {
            return Err(
                ectx!(err ErrorContext::InvalidTransaction, ErrorKind::Internal => tx.clone(), dr_account.clone(), cr_account.clone()),
            );
        }
        let balance = self
            .transactions_repo
            .get_accounts_balance(tx.user_id, &[dr_account])
            .map(|accounts| accounts[0].balance)
            .map_err(ectx!(try convert => tx.clone()))?;
        if balance >= tx.value {
            self.transactions_repo.create(tx.clone()).map_err(ectx!(convert => tx.clone()))
        } else {
            Err(ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::Balance => tx))
        }
    }

    fn create_internal_mono_currency_tx(
        &self,
        create_tx_input: CreateTransactionInput,
        dr_account: Account,
        cr_account: Account,
    ) -> Result<Transaction, Error> {
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
        };
        self.create_base_tx(tx, dr_account, cr_account)
    }

    fn create_external_mono_currency_tx(
        &self,
        input: CreateTransactionInput,
        from_account: Account,
        to_blockchain_address: BlockchainAddress,
        to_currency: Currency,
        // these group params will be filled with defaults for external mono currency
        // to reuse it in external withdrawal, we put overrides here
        tx_kind: Option<TransactionKind>,
        tx_group_kind: Option<TransactionGroupKind>,
    ) -> Result<Vec<Transaction>, Error> {
        if from_account.currency != to_currency {
            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => from_account, to_blockchain_address, to_currency));
        };

        let value = input.value;
        let FeeEstimate {
            total_fee: total_fee_est,
            fee_price: fee_price_est,
        } = self.blockchain_service.estimate_withdrawal_fee_price(input.fee, to_currency)?;
        let withdrawal_accs_with_balance = self
            .transactions_repo
            .get_accounts_for_withdrawal(value, to_currency, input.user_id, total_fee_est)
            .map_err(ectx!(try convert ErrorContext::NotEnoughFunds => value, to_currency, input.user_id))?;

        let mut total_value = Amount::new(0);
        //double check
        for AccountWithBalance {
            account: acc,
            balance: value,
        } in &withdrawal_accs_with_balance
        {
            let acc_id = acc.id;
            let balance = self
                .transactions_repo
                .get_account_balance(acc_id, AccountKind::Dr)
                .map_err(ectx!(try convert => acc_id))?;
            if balance < *value {
                return Err(ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::Balance => balance, value));
            }
            total_value = total_value
                .checked_add(*value)
                .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        }

        if total_value != input.value {
            return Err(ectx!(ErrorContext::InvalidValue, ErrorKind::Internal => input.clone()));
        }

        let mut res: Vec<Transaction> = Vec::new();
        let mut current_tx_id = input.id;
        let fees_account = self.system_service.get_system_fees_account(to_currency)?;

        for AccountWithBalance {
            account: acc,
            balance: value,
        } in &withdrawal_accs_with_balance
        {
            let to = to_blockchain_address.clone();
            let blockchain_tx_id_res = match to_currency {
                Currency::Eth => self
                    .blockchain_service
                    .create_ethereum_tx(acc.address.clone(), to, *value, fee_price_est, Currency::Eth),
                Currency::Stq => self
                    .blockchain_service
                    .create_ethereum_tx(acc.address.clone(), to, *value, fee_price_est, Currency::Stq),
                Currency::Btc => self
                    .blockchain_service
                    .create_bitcoin_tx(acc.address.clone(), to, *value, fee_price_est),
            };

            match blockchain_tx_id_res {
                Ok(blockchain_tx_id) => {
                    let new_tx = NewTransaction {
                        id: current_tx_id,
                        gid: input.id,
                        user_id: input.user_id,
                        dr_account_id: from_account.id,
                        cr_account_id: acc.id,
                        currency: to_currency,
                        value: *value,
                        status: TransactionStatus::Pending,
                        blockchain_tx_id: Some(blockchain_tx_id),
                        kind: tx_kind.unwrap_or(TransactionKind::Withdrawal),
                        group_kind: tx_group_kind.unwrap_or(TransactionGroupKind::Withdrawal),
                        related_tx: None,
                    };
                    res.push(self.create_base_tx(new_tx, from_account.clone(), acc.clone())?);
                    current_tx_id = current_tx_id.next();
                }
                // Note - we don't do early exit here, since we need to complete our transaction with previously
                // written transactions
                Err(e) => {
                    if res.len() == 0 {
                        // didn't write any transaction to blockchain, so safe to abort
                        return Err(ectx!(err e, ErrorKind::Internal));
                    } else {
                        // partial write of some transactions, cannot abort, just logging error and break cycle
                        log_and_capture_error(e.compat());
                        break;
                    }
                }
            }
        }
        let fee_tx = NewTransaction {
            id: current_tx_id,
            gid: input.id,
            user_id: input.user_id,
            dr_account_id: from_account.id,
            cr_account_id: fees_account.id,
            currency: to_currency,
            value: input.fee,
            status: TransactionStatus::Done,
            blockchain_tx_id: None,
            kind: TransactionKind::Fee,
            group_kind: tx_group_kind.unwrap_or(TransactionGroupKind::Withdrawal),
            related_tx: None,
        };
        res.push(self.create_base_tx(fee_tx, from_account.clone(), fees_account.clone())?);

        Ok(res)
    }

    fn create_internal_multi_currency_tx(
        &self,
        input: CreateTransactionInput,
        from_account: Account,
        to_account: Account,
        exchange_id: ExchangeId,
        exchange_rate: f64,
    ) -> Result<Vec<Transaction>, Error> {
        let mut result: Vec<Transaction> = Vec::new();

        let (from_value, to_value) = if from_account.currency == input.value_currency {
            (input.value, input.value.convert(from_account.currency, exchange_rate))
        } else if to_account.currency == input.value_currency {
            (input.value.convert(to_account.currency, 1.0 / exchange_rate), input.value)
        } else {
            panic!(
                "Unexpected currency. Input: {:#?}, from_account: {:#?}, to_account: {:#?}",
                input, from_account, to_account
            )
        };

        // Moving money from `from` account to system liquidity account
        let from_counterpart_acc = self.get_system_liquidity_account(from_account.currency)?;
        let from_input = CreateTransactionInput {
            value: from_value,
            ..input.clone()
        };
        let txs = self.create_internal_mono_currency_tx(
            from_input,
            from_account.clone(),
            from_counterpart_acc,
            None,
            TransactionStatus::Done,
            input.id,
            TransactionKind::MultiFrom,
            TransactionGroupKind::InternalMulti,
            None,
        )?;
        result.extend(txs.into_iter());

        // Moving money from system liquidity account to `to` account
        let tx_next_id = input.id.next();
        let to_input = CreateTransactionInput {
            id: tx_next_id,
            value: to_value,
            ..input.clone()
        };
        let to_counterpart_acc = self.get_system_liquidity_account(to_account.currency)?;
        let txs = self.create_internal_mono_currency_tx(
            to_input,
            to_counterpart_acc,
            to_account.clone(),
            None,
            TransactionStatus::Done,
            input.id,
            TransactionKind::MultiTo,
            TransactionGroupKind::InternalMulti,
            None,
        )?;
        result.extend(txs.into_iter());

        let exchange_input = ExchangeInput {
            id: exchange_id,
            from: from_account.currency,
            to: to_account.currency,
            rate: exchange_rate,
            actual_amount: input.value,
            amount_currency: input.value_currency,
        };
        let exchange_input_clone = exchange_input.clone();
        let _ = self
            .exchange_client
            .exchange(exchange_input, Role::User)
            .map_err(ectx!(try convert => exchange_input_clone))
            .wait()?;

        Ok(result)
    }
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionInput,
    ) -> Box<Future<Item = TransactionOut, Error = Error> + Send> {
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let input_clone = input.clone();
        Box::new(self.auth_service.authenticate(token.clone()).and_then(move |user| {
            db_executor.execute_transaction_with_isolation(Isolation::Serializable, move || {
                let mut core = Core::new().unwrap();
                let tx_type = self_clone.validate_and_classify_transaction(&input)?;
                let f = future::lazy(|| {
                    let tx_group = match tx_type {
                        TransactionType::Internal(from_account, to_account) => self_clone.create_internal_mono_currency_tx(
                            input.clone(),
                            from_account,
                            to_account,
                            None,
                            TransactionStatus::Done,
                            input.id,
                            TransactionKind::Internal,
                            TransactionGroupKind::Internal,
                            None,
                        ),
                        TransactionType::Withdrawal(from_account, to_account_address, currency) => {
                            self_clone.create_external_mono_currency_tx(user.id, input, from_account, to_account_address, currency)
                        }
                        TransactionType::InternalExchange(from, to, exchange_id, rate) => {
                            self_clone.create_internal_multi_currency_tx(input, from, to, exchange_id, rate)
                        }
                        _ => return Err(ectx!(err ErrorContext::NotSupported, ErrorKind::MalformedInput => tx_type, input_clone)),
                    }?;
                    self_clone.convert_transaction(tx_group)
                });
                core.run(f)
            })
        }))
    }

    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<TransactionOut>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let service = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let transaction = transactions_repo
                    .get(transaction_id)
                    .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
                if let Some(ref transaction) = transaction {
                    if transaction.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    let tx_group = transactions_repo
                        .get_by_gid(transaction.gid)
                        .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
                    let tx_out = service.convert_transaction(tx_group)?;
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
                if let Some(mut account) = account {
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
        let service = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || -> Result<Vec<TransactionOut>, Error> {
                if user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                let txs = transactions_repo
                    .list_for_user(user_id, offset, limit * MAX_TRANSACTIONS_PER_TRANSACTION_OUT)
                    .map_err(ectx!(try convert => user_id, offset, limit))?;
                group_transactions(&txs)
                    .into_iter()
                    .map(|tx_group| service.convert_transaction(tx_group))
                    .take(limit as usize)
                    .collect()
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
        let service = self.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let account = accounts_repo
                    .get(account_id)
                    .map_err(ectx!(try ErrorKind::Internal => account_id))?;
                if let Some(ref account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                } else {
                    return Err(ectx!(err ErrorContext::NoAccount, ErrorKind::NotFound => account_id));
                }
                let txs = transactions_repo
                    .list_for_account(account_id, offset, limit * MAX_TRANSACTIONS_PER_TRANSACTION_OUT)
                    .map_err(ectx!(try convert => account_id))?;
                group_transactions(&txs)
                    .into_iter()
                    .map(|tx_group| service.convert_transaction(tx_group))
                    .take(limit as usize)
                    .collect()
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

fn fold_statuses(statuses: &[TransactionStatus]) -> TransactionStatus {
    statuses.into_iter().fold(TransactionStatus::Done, |acc, elem| {
        if (acc == TransactionStatus::Pending) || (*elem == TransactionStatus::Pending) {
            return TransactionStatus::Pending;
        }
        acc
    })
}
