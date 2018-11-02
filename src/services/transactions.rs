use std::sync::Arc;
use utils::log_error;

use futures::future::{self, Either};
use futures::prelude::*;
use futures::stream::iter_ok;
use futures::IntoFuture;
use validator::Validate;

use super::auth::AuthService;
use super::error::*;
use client::BlockchainClient;
use client::KeysClient;
use models::*;
use prelude::*;
use repos::{AccountsRepo, BlockchainTransactionsRepo, DbExecutor, Isolation, PendingBlockchainTransactionsRepo, TransactionsRepo};
use tokio_core::reactor::Core;
use utils::log_and_capture_error;

#[derive(Clone)]
pub struct TransactionsServiceImpl<E: DbExecutor> {
    auth_service: Arc<dyn AuthService>,
    transactions_repo: Arc<dyn TransactionsRepo>,
    pending_transactions_repo: Arc<dyn PendingBlockchainTransactionsRepo>,
    blockchain_transactions_repo: Arc<dyn BlockchainTransactionsRepo>,
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    keys_client: Arc<dyn KeysClient>,
    blockchain_client: Arc<dyn BlockchainClient>,
}

#[derive(Debug, Clone)]
enum TransactionType {
    Internal(Account, Account),
    Withdrawal(Account, AccountAddress, Currency),
    InternalExchange(Account, Account),
    WithdrawalExchange(Account, AccountAddress, Currency),
}

pub trait TransactionsService: Send + Sync + 'static {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionInput,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send>;
    // fn create_transaction_local(&self, input: CreateTransactionLocal) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    // fn deposit_funds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    // fn withdraw(&self, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<TransactionOut>, Error = Error> + Send>;
    fn get_account_balance(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send>;
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
    // fn create_transaction_ethereum(
    //     &self,
    //     from: AccountAddress,
    //     to: AccountAddress,
    //     value: Amount,
    //     fee: Amount,
    //     currency: Currency,
    // ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send>;
    // fn create_transaction_bitcoin(
    //     &self,
    //     from: AccountAddress,
    //     to: AccountAddress,
    //     value: Amount,
    //     fee: Amount,
    // ) -> Box<Future<Item = BlockchainTransactionId, Error = Error> + Send>;
    // fn convert_transaction(&self, transaction: Transaction) -> Box<Future<Item = TransactionOut, Error = Error> + Send>;
}

impl<E: DbExecutor> TransactionsServiceImpl<E> {
    pub fn new(
        auth_service: Arc<AuthService>,
        transactions_repo: Arc<TransactionsRepo>,
        pending_transactions_repo: Arc<dyn PendingBlockchainTransactionsRepo>,
        blockchain_transactions_repo: Arc<dyn BlockchainTransactionsRepo>,
        accounts_repo: Arc<dyn AccountsRepo>,
        db_executor: E,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
    ) -> Self {
        Self {
            auth_service,
            transactions_repo,
            pending_transactions_repo,
            blockchain_transactions_repo,
            accounts_repo,
            db_executor,
            keys_client,
            blockchain_client,
        }
    }

    fn validate_and_classify_transaction(&self, input: &CreateTransactionInput) -> Result<TransactionType, Error> {
        input
            .validate()
            .map_err(|e| ectx!(try err e.clone(), ErrorKind::InvalidInput(e) => input))?;
        let from_account = self
            .accounts_repo
            .get(input.from)?
            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => input))?;

        match input.to_type {
            ReceiptType::Account => {
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
                    Ok(TransactionType::InternalExchange(from_account, to_account))
                } else {
                    Ok(TransactionType::Internal(from_account, to_account))
                }
            }
            ReceiptType::Address => {
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
                            Ok(TransactionType::WithdrawalExchange(from_account, to_address, input.to_currency))
                        } else {
                            Ok(TransactionType::Withdrawal(from_account, to_address, input.to_currency))
                        }
                    }
                    Some(to_account) => {
                        if from_account.currency != to_account.currency {
                            Ok(TransactionType::InternalExchange(from_account, to_account))
                        } else {
                            Ok(TransactionType::Internal(from_account, to_account))
                        }
                    }
                }
            }
        }
    }

    fn create_external_mono_currency_tx(
        &self,
        user_id: UserId,
        input: CreateTransactionInput,
        from_account: Account,
        to_account_address: AccountAddress,
        currency: Currency,
    ) -> Result<Vec<Transaction>, Error> {
        if from_account.currency != currency {
            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => from_account, to_account_address, currency));
        };

        let value = input.value;
        let withdrawal_accs_and_vals = self
            .transactions_repo
            .get_with_enough_value(value, currency, user_id)
            .map_err(ectx!(try convert ErrorContext::NotEnoughFunds => value, currency, user_id))?;

        //double check
        for (acc, needed_amount) in &withdrawal_accs_and_vals {
            let acc_id = acc.id;
            let balance = self
                .transactions_repo
                .get_account_balance(acc_id, AccountKind::Dr)
                .map_err(ectx!(try convert => acc_id))?;
            if balance < *needed_amount {
                return Err(ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::Balance => balance, needed_amount));
            }
        }

        let mut res: Vec<Transaction> = Vec::new();

        for (acc, value) in &withdrawal_accs_and_vals {
            let to = to_account_address.clone();
            // Todo this fee is ineffective, since keys client take system's fee
            let fee = Amount::new(0);
            // Note - we don't do early exit here, since we need to complete our transaction with previously
            // written transactions
            let blockchain_tx_id_res = match currency {
                Currency::Eth => self.create_ethereum_tx(acc.address.clone(), to, *value, fee, Currency::Eth),
                Currency::Stq => self.create_ethereum_tx(acc.address.clone(), to, *value, fee, Currency::Stq),
                Currency::Btc => self.create_bitcoin_tx(acc.address.clone(), to, *value, fee),
            };

            match blockchain_tx_id_res {
                Ok(tx_id) => {
                    let txs = self.create_internal_mono_currency_tx(input.clone(), from_account.clone(), acc.clone(), Some(tx_id))?;
                    res.extend(txs.into_iter());
                }
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
        Ok(res)
    }

    fn create_internal_mono_currency_tx(
        &self,
        input: CreateTransactionInput,
        from_account: Account,
        to_account: Account,
        blockchain_tx_id: Option<BlockchainTransactionId>,
    ) -> Result<Vec<Transaction>, Error> {
        if from_account.currency != to_account.currency {
            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => from_account, to_account));
        }
        let from_account_id = from_account.id;
        let balance = self
            .transactions_repo
            .get_account_balance(from_account_id, AccountKind::Cr)
            .map_err(ectx!(try convert => from_account_id))?;
        if balance >= input.value {
            let new_transaction = NewTransaction {
                id: input.id,
                user_id: input.user_id,
                dr_account_id: from_account.id,
                cr_account_id: to_account.id,
                currency: input.to_currency,
                value: input.value,
                hold_until: input.hold_until,
                status: TransactionStatus::Done,
                blockchain_tx_id,
                // Todo - fees
                fee: Amount::default(),
            };
            self.transactions_repo
                .create(new_transaction.clone())
                .map(|tx| vec![tx])
                .map_err(ectx!(convert => new_transaction))
        } else {
            Err(ectx!(err ErrorContext::NotEnoughFunds, ErrorKind::Balance => balance, input.value))
        }
    }

    fn create_bitcoin_tx(
        &self,
        from: AccountAddress,
        to: AccountAddress,
        value: Amount,
        fee: Amount,
    ) -> Result<BlockchainTransactionId, Error> {
        let from_clone = from.clone();
        let utxos = self
            .blockchain_client
            .get_bitcoin_utxos(from.clone())
            .map_err(ectx!(try convert => from_clone))
            .wait()?;

        let create_blockchain_input = CreateBlockchainTx::new(from, to, Currency::Btc, value, fee, None, Some(utxos));
        let create_blockchain_input_clone = create_blockchain_input.clone();

        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone())
            .map_err(ectx!(try convert => create_blockchain_input_clone))
            .wait()?;

        let blockchain_tx_id = self
            .blockchain_client
            .post_bitcoin_transaction(raw_tx)
            .map_err(ectx!(try convert))
            .wait()?;

        let new_pending = (create_blockchain_input, blockchain_tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };

        Ok(blockchain_tx_id)
    }

    fn create_ethereum_tx(
        &self,
        from: AccountAddress,
        to: AccountAddress,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Result<BlockchainTransactionId, Error> {
        match currency {
            Currency::Eth => (),
            Currency::Stq => (),
            _ => return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal)),
        }
        let from_clone = from.clone();
        let nonce = self
            .blockchain_client
            .get_ethereum_nonce(from_clone.clone())
            .map_err(ectx!(try convert => from_clone))
            .wait()?;

        // creating blockchain transactions array
        let create_blockchain_input = CreateBlockchainTx::new(from, to, currency, value, fee, Some(nonce), None);

        let create_blockchain = create_blockchain_input.clone();
        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone())
            .map_err(ectx!(try convert => create_blockchain_input))
            .wait()?;
        let tx_id = self
            .blockchain_client
            .post_ethereum_transaction(raw_tx)
            .map_err(ectx!(try convert))
            .wait()?;

        let tx_id = match currency {
            Currency::Eth => tx_id,
            // Erc-20 token, we need event log number here, to make a tx_id unique
            _ => BlockchainTransactionId::new(format!("{}:0", tx_id)),
        };
        let new_pending = (create_blockchain, tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };
        Ok(tx_id)
    }
    fn convert_transaction(&self, transaction: Transaction) -> Box<Future<Item = TransactionOut, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let pending_transactions_repo = self.pending_transactions_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        let transaction_id = transaction.id;
        Box::new(db_executor.execute(move || {
            let cr_account = accounts_repo
                .get(transaction.cr_account_id)
                .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
            let cr_account_id = transaction.cr_account_id;
            let cr_account = cr_account.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => cr_account_id))?;

            let dr_account = accounts_repo
                .get(transaction.dr_account_id)
                .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
            let dr_account_id = transaction.dr_account_id;
            let dr_account = dr_account.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => dr_account_id))?;

            if cr_account.kind == AccountKind::Cr && dr_account.kind == AccountKind::Cr {
                let from = TransactionAddressInfo::new(Some(dr_account.id), dr_account.address);
                let to = TransactionAddressInfo::new(Some(cr_account.id), cr_account.address);
                Ok(TransactionOut::new(&transaction, vec![from], to))
            } else if cr_account.kind == AccountKind::Cr && dr_account.kind == AccountKind::Dr {
                let hash = transaction
                    .blockchain_tx_id
                    .clone()
                    .ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => transaction_id))?;
                let to = TransactionAddressInfo::new(Some(cr_account.id), cr_account.address);

                let hash_clone = hash.clone();
                let hash_clone2 = hash.clone();
                let hash_clone3 = hash.clone();
                if let Some(pending_transaction) = pending_transactions_repo
                    .get(hash.clone())
                    .map_err(ectx!(try convert => hash_clone))?
                {
                    let from = TransactionAddressInfo::new(None, pending_transaction.from_);
                    Ok(TransactionOut::new(&transaction, vec![from], to))
                } else if let Some(blockchain_transaction_db) = blockchain_transactions_repo
                    .get(hash.clone())
                    .map_err(ectx!(try convert => hash_clone2))?
                {
                    let blockchain_transaction: BlockchainTransaction = blockchain_transaction_db.into();
                    let (froms, _) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert => hash))?;
                    let from = froms
                        .into_iter()
                        .map(|address| TransactionAddressInfo::new(None, address))
                        .collect();
                    Ok(TransactionOut::new(&transaction, from, to))
                } else {
                    return Err(ectx!(err ErrorContext::NoTransaction, ErrorKind::NotFound => hash_clone3));
                }
            } else if cr_account.kind == AccountKind::Dr && dr_account.kind == AccountKind::Cr {
                let hash = transaction
                    .blockchain_tx_id
                    .clone()
                    .ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => transaction_id))?;
                let from = TransactionAddressInfo::new(Some(dr_account.id), dr_account.address);

                let hash_clone = hash.clone();
                let hash_clone2 = hash.clone();
                let hash_clone3 = hash.clone();
                if let Some(pending_transaction) = pending_transactions_repo
                    .get(hash.clone())
                    .map_err(ectx!(try convert => hash_clone))?
                {
                    let to = TransactionAddressInfo::new(None, pending_transaction.to_);
                    Ok(TransactionOut::new(&transaction, vec![from], to))
                } else if let Some(blockchain_transaction_db) = blockchain_transactions_repo
                    .get(hash.clone())
                    .map_err(ectx!(try convert => hash_clone2))?
                {
                    let hash_clone4 = hash.clone();
                    let blockchain_transaction: BlockchainTransaction = blockchain_transaction_db.into();
                    let (_, to_s) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert => hash_clone4))?;
                    let to = to_s
                        .into_iter()
                        .map(|(address, _)| TransactionAddressInfo::new(None, address))
                        .nth(0);
                    let to = to.ok_or_else(|| ectx!(try err ErrorContext::NoTransaction, ErrorKind::NotFound => hash))?;
                    Ok(TransactionOut::new(&transaction, vec![from], to))
                } else {
                    return Err(ectx!(err ErrorContext::NoTransaction, ErrorKind::NotFound => hash_clone3));
                }
            } else {
                return Err(ectx!(err ErrorContext::InvalidTransaction, ErrorKind::Internal => transaction_id));
            }
        }))
    }
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionInput,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send> {
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let input_clone = input.clone();
        Box::new(self.auth_service.authenticate(token.clone()).and_then(move |user| {
            db_executor
                .execute_transaction_with_isolation(Isolation::Serializable, move || {
                    let mut core = Core::new().unwrap();
                    let tx_type = self_clone.validate_and_classify_transaction(&input)?;
                    let f = future::lazy(|| match tx_type {
                        TransactionType::Internal(from_account, to_account) => {
                            self_clone.create_internal_mono_currency_tx(input, from_account, to_account, None)
                        }
                        TransactionType::Withdrawal(from_account, to_account_address, currency) => {
                            self_clone.create_external_mono_currency_tx(user.id, input, from_account, to_account_address, currency)
                        }
                        _ => return Err(ectx!(err ErrorContext::NotSupported, ErrorKind::MalformedInput => tx_type, input_clone)),
                    });
                    core.run(f)
                }).and_then(move |txs| {
                    let fs: Vec<_> = txs.into_iter().map(move |tx| self_clone2.convert_transaction(tx)).collect();
                    future::join_all(fs)
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
            db_executor
                .execute(move || {
                    let transaction = transactions_repo
                        .get(transaction_id)
                        .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
                    if let Some(ref transaction) = transaction {
                        if transaction.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }
                    }
                    Ok(transaction)
                }).and_then(move |transaction| {
                    if let Some(transaction) = transaction {
                        Either::A(service.convert_transaction(transaction).map(Some))
                    } else {
                        Either::B(future::ok(None))
                    }
                })
        }))
    }
    fn get_account_balance(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let account = accounts_repo.get(account_id).map_err(ectx!(try convert => account_id))?;
                if let Some(mut account) = account {
                    if account.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    let balance = transactions_repo
                        .get_account_balance(account_id, account.kind)
                        .map_err(ectx!(try convert => account_id))?;
                    account.balance = balance;
                    Ok(account)
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
            db_executor
                .execute(move || {
                    if user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                    transactions_repo
                        .list_for_user(user_id, offset, limit)
                        .map_err(ectx!(convert => user_id, offset, limit))
                }).and_then(|transactions| {
                    iter_ok::<_, Error>(transactions).fold(vec![], move |mut transactions, transaction| {
                        service.convert_transaction(transaction).and_then(|res| {
                            transactions.push(res);
                            Ok(transactions) as Result<Vec<TransactionOut>, Error>
                        })
                    })
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
            db_executor
                .execute(move || {
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
                    transactions_repo
                        .list_for_account(account_id, offset, limit)
                        .map_err(ectx!(convert => account_id))
                }).and_then(|transactions| {
                    iter_ok::<_, Error>(transactions).fold(vec![], move |mut transactions, transaction| {
                        service.convert_transaction(transaction).and_then(|res| {
                            transactions.push(res);
                            Ok(transactions) as Result<Vec<TransactionOut>, Error>
                        })
                    })
                })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::*;
    use repos::*;
    use services::*;
    use tokio_core::reactor::Core;

    fn create_services(
        token: AuthenticationToken,
        user_id: UserId,
    ) -> (AccountsServiceImpl<DbExecutorMock>, TransactionsServiceImpl<DbExecutorMock>) {
        let auth_service = Arc::new(AuthServiceMock::new(vec![(token, user_id)]));
        let accounts_repo = Arc::new(AccountsRepoMock::default());
        let transactions_repo = Arc::new(TransactionsRepoMock::default());
        let pending_transactions_repo = Arc::new(PendingBlockchainTransactionsRepoMock::default());
        let blockchain_transactions_repo = Arc::new(BlockchainTransactionsRepoMock::default());
        let keys_client = Arc::new(KeysClientMock::default());
        let blockchain_client = Arc::new(BlockchainClientMock::default());
        let db_executor = DbExecutorMock::default();
        let acc_service = AccountsServiceImpl::new(
            auth_service.clone(),
            accounts_repo.clone(),
            db_executor.clone(),
            keys_client.clone(),
        );
        let trans_service = TransactionsServiceImpl::new(
            auth_service,
            transactions_repo,
            pending_transactions_repo,
            blockchain_transactions_repo,
            accounts_repo,
            db_executor,
            keys_client,
            blockchain_client,
        );
        (acc_service, trans_service)
    }

    //     #[test]
    //     fn test_transaction_create() {
    //         let mut core = Core::new().unwrap();
    //         let token = AuthenticationToken::default();
    //         let user_id = UserId::generate();
    //         let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //         let mut dr_account = CreateAccount::default();
    //         dr_account.name = "test test test acc".to_string();
    //         dr_account.user_id = user_id;
    //         let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

    //         let mut new_transaction = DepositFounds::default();
    //         new_transaction.value = Amount::new(100501);
    //         new_transaction.address = dr_account.address.clone();

    //         core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

    // ;        let mut cr_account = CreateAccount::default();
    //         cr_account.name = "test test test acc".to_string();
    //         cr_account.user_id = user_id;
    //         let cr_account = core.run(acc_service.create_account(token.clone(), cr_account.clone())).unwrap();

    //         let mut new_transaction = CreateTransactionLocal::default();
    //         new_transaction.value = Amount::new(100500);
    //         new_transaction.cr_account = cr_account;
    //         new_transaction.dr_account = dr_account;

    //         let transaction = core.run(trans_service.create_transaction_local(new_transaction));
    //         assert!(transaction.is_ok());
    //     }
    // #[test]
    // fn test_transaction_get_by_id() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //     let mut cr_account = CreateAccount::default();
    //     cr_account.name = "test test test acc".to_string();
    //     cr_account.user_id = user_id;

    //     let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

    //     let mut new_transaction = DepositFounds::default();
    //     new_transaction.value = Amount::new(100500);
    //     new_transaction.address = cr_account.address;
    //     new_transaction.user_id = user_id;

    //     let transaction = core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();
    //     let transaction = core.run(trans_service.get_transaction(token, transaction.id));
    //     assert!(transaction.is_ok());
    // }
    // #[test]
    // fn test_transaction_get_for_users() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //     let mut cr_account = CreateAccount::default();
    //     cr_account.name = "test test test acc".to_string();
    //     cr_account.user_id = user_id;

    //     let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

    //     let mut new_transaction = DepositFounds::default();
    //     new_transaction.value = Amount::new(100500);
    //     new_transaction.address = cr_account.address;
    //     new_transaction.user_id = user_id;

    //     let _ = core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

    //     let transactions = core.run(trans_service.get_transactions_for_user(token, user_id, 0, 10));
    //     assert!(transactions.is_ok());
    //     assert_eq!(transactions.unwrap().len(), 1);
    // }
    // #[test]
    // fn test_transaction_get_for_account() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //     let mut dr_account = CreateAccount::default();
    //     dr_account.name = "test test test acc".to_string();
    //     dr_account.user_id = user_id;
    //     let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

    //     let mut new_transaction = DepositFounds::default();
    //     new_transaction.value = Amount::new(100501);
    //     new_transaction.address = dr_account.address.clone();

    //     core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

    //     let mut cr_account = CreateAccount::default();
    //     cr_account.name = "test test test acc".to_string();
    //     cr_account.user_id = user_id;
    //     let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

    //     let mut new_transaction = CreateTransactionLocal::default();
    //     new_transaction.value = Amount::new(100500);
    //     new_transaction.cr_account = cr_account;
    //     new_transaction.dr_account = dr_account;

    //     let transaction = core.run(trans_service.create_transaction_local(new_transaction)).unwrap();
    //     let transaction = core.run(trans_service.get_account_transactions(token, transaction.cr_account_id, 0, 10));
    //     assert!(transaction.is_ok());
    // }
    // #[test]
    // fn test_transaction_deposit_funds() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);
    //     let mut cr_account = CreateAccount::default();
    //     cr_account.name = "test test test acc".to_string();
    //     cr_account.user_id = user_id;

    //     let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

    //     let mut new_transaction = DepositFounds::default();
    //     new_transaction.value = Amount::new(100500);
    //     new_transaction.address = cr_account.address;

    //     let transaction = core.run(trans_service.deposit_funds(token.clone(), new_transaction));
    //     assert!(transaction.is_ok());
    // }
    // #[test]
    // fn test_transaction_withdraw() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //     //creating withdraw account
    //     let mut dr_account = CreateAccount::default();
    //     dr_account.name = "test test test acc".to_string();
    //     dr_account.user_id = user_id;
    //     let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

    //     //depositing on withdraw account
    //     let mut deposit = DepositFounds::default();
    //     deposit.value = Amount::new(100500);
    //     deposit.address = dr_account.address.clone();

    //     core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

    //     //creating random account
    //     let mut cr_account = CreateAccount::default();
    //     cr_account.name = "test test test acc".to_string();
    //     cr_account.user_id = user_id;
    //     let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

    //     //depositin on random account
    //     let mut deposit = DepositFounds::default();
    //     deposit.value = Amount::new(100500);
    //     deposit.address = cr_account.address;

    //     core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

    //     //withdrawing
    //     let mut withdraw = Withdraw::default();
    //     withdraw.value = Amount::new(100);
    //     withdraw.dr_account = dr_account;

    //     let transaction = core.run(trans_service.withdraw(withdraw));
    //     assert!(transaction.is_ok());
    // }
    // #[test]
    // fn test_account_get_balance() {
    //     let mut core = Core::new().unwrap();
    //     let token = AuthenticationToken::default();
    //     let user_id = UserId::generate();
    //     let (acc_service, trans_service) = create_services(token.clone(), user_id);

    //     let mut new_account = CreateAccount::default();
    //     new_account.name = "test test test acc".to_string();
    //     new_account.user_id = user_id;

    //     core.run(acc_service.create_account(token.clone(), new_account.clone())).unwrap();

    //     let account = core.run(trans_service.get_account_balance(token, new_account.id));
    //     assert!(account.is_ok());
    // }
}
