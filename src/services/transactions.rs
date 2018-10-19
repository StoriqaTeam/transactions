use std::sync::Arc;

use futures::future::Either;
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
use repos::{AccountsRepo, DbExecutor, TransactionsRepo};
use utils::log_and_capture_error;

#[derive(Clone)]
pub struct TransactionsServiceImpl<E: DbExecutor> {
    auth_service: Arc<dyn AuthService>,
    transactions_repo: Arc<dyn TransactionsRepo>,
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    keys_client: Arc<dyn KeysClient>,
    blockchain_client: Arc<dyn BlockchainClient>,
}

impl<E: DbExecutor> TransactionsServiceImpl<E> {
    pub fn new(
        auth_service: Arc<AuthService>,
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<dyn AccountsRepo>,
        db_executor: E,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
    ) -> Self {
        Self {
            auth_service,
            transactions_repo,
            accounts_repo,
            db_executor,
            keys_client,
            blockchain_client,
        }
    }
}

pub trait TransactionsService: Send + Sync + 'static {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransaction,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn create_transaction_local(&self, input: CreateTransactionLocal) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn deposit_funds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<Transaction>, Error = Error> + Send>;
    fn get_account_balance(&self, token: AuthenticationToken, account_id: AccountId) -> Box<Future<Item = Account, Error = Error> + Send>;
    fn get_transactions_for_user(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        offset: TransactionId,
        limit: i64,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn get_account_transactions(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn create_transaction_ethereum(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        dr_acc: Account,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn create_transaction_bitcoin(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        dr_acc: Account,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransaction,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let service = self.clone();
        Box::new(self.auth_service.authenticate(token.clone()).and_then(move |user| {
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then({
                    let input = input.clone();
                    move |_| {
                        db_executor.execute(move || {
                            // check that dr account exists and it is belonging to one user
                            let dr_account_id = input.dr_account_id;
                            let dr_acc = accounts_repo
                                .get(dr_account_id)
                                .map_err(ectx!(try ErrorKind::Internal => dr_account_id))?;
                            let dr_acc =
                                dr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => dr_account_id))?;
                            if dr_acc.user_id != user.id {
                                return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                            }

                            // check that cr account exists and it is belonging to one user
                            let to = input.to.clone();
                            let input_type = input.to_type.clone();
                            match input_type {
                                ReceiptType::Account => {
                                    let cr_account_id = to.clone().to_account_id().map_err(
                                        move |_| ectx!(try err ErrorKind::MalformedInput, ErrorKind::MalformedInput => to, input_type),
                                    )?;
                                    let cr_acc = accounts_repo
                                        .get(cr_account_id)
                                        .map_err(ectx!(try ErrorKind::Internal => cr_account_id))?;
                                    let cr_acc = cr_acc
                                        .ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => cr_account_id))?;
                                    if cr_acc.user_id != user.id {
                                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                                    }
                                    Ok((dr_acc, CrReceiptType::Account(cr_acc)))
                                }
                                ReceiptType::Address => {
                                    let cr_account_address = to.to_account_address();
                                    let cr_account_address_clone = cr_account_address.clone();
                                    let cr_acc = accounts_repo
                                        .get_by_address(cr_account_address.clone(), AccountKind::Cr)
                                        .map_err(ectx!(try ErrorKind::Internal => cr_account_address))?;
                                    if let Some(cr_acc) = cr_acc {
                                        if cr_acc.user_id != user.id {
                                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                                        }
                                        Ok((dr_acc, CrReceiptType::Account(cr_acc)))
                                    } else {
                                        Ok((dr_acc, CrReceiptType::Address(cr_account_address_clone)))
                                    }
                                }
                            }
                        })
                    }
                }).and_then(move |(dr_acc, cr_acc)| match cr_acc {
                    CrReceiptType::Account(cr_acc) => Either::A(
                        service
                            .create_transaction_local(CreateTransactionLocal::new(&input, dr_acc, cr_acc))
                            .map(|tr| vec![tr]),
                    ),
                    CrReceiptType::Address(cr_account_address) => {
                        Either::B(service.withdraw(token, Withdraw::new(&input, dr_acc, cr_account_address)))
                    }
                })
        }))
    }

    fn create_transaction_local(&self, input: CreateTransactionLocal) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute_transaction(move || {
                        // check that balance > value of input
                        let dr_account_id = input.dr_account.id;
                        let balance = transactions_repo
                            .get_account_balance(dr_account_id)
                            .map_err(ectx!(try convert => dr_account_id))?;

                        if balance >= input.value {
                            let new_transaction: NewTransaction = NewTransaction::from_local(&input);
                            transactions_repo
                                .create(new_transaction.clone())
                                .map_err(ectx!(convert => new_transaction))
                        } else {
                            Err(ectx!(err ErrorContext::NotEnoughFounds, ErrorKind::Balance => balance, input.value))
                        }
                    })
                }),
        )
    }
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let currency = input.currency;
        let user_id = input.user_id;
        let fee = input.fee;
        let service = self.clone();
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute(move || {
                        // check that cr account exists and it is belonging to one user
                        let currency = input.currency;
                        if input.dr_account.currency != currency {
                            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Balance => currency));
                        }
                        let value = input.value;
                        let dr_account_id = input.dr_account.id;
                        let balance = transactions_repo
                            .get_account_balance(dr_account_id)
                            .map_err(ectx!(try convert => dr_account_id))?;
                        if balance < value {
                            return Err(ectx!(err ErrorContext::NotEnoughFounds, ErrorKind::Balance => value));
                        }

                        accounts_repo
                            .get_with_enough_value(value, currency, user_id)
                            .map_err(ectx!(convert ErrorContext::NotEnoughFounds => value, currency, user_id))
                            .map(|cr_accs| (input.dr_account, cr_accs))
                    })
                }).and_then(move |(dr_acc, cr_accs)| {
                    iter_ok::<_, Error>(cr_accs).fold(vec![], move |mut transactions, (cr_acc, value)| {
                        match currency {
                            Currency::Eth => service.create_transaction_ethereum(
                                token.clone(),
                                user_id,
                                dr_acc.clone(),
                                cr_acc,
                                value,
                                fee,
                                Currency::Eth,
                            ),
                            Currency::Stq => service.create_transaction_ethereum(
                                token.clone(),
                                user_id,
                                dr_acc.clone(),
                                cr_acc,
                                value,
                                fee,
                                Currency::Stq,
                            ),
                            Currency::Btc => service.create_transaction_bitcoin(token.clone(), user_id, dr_acc.clone(), cr_acc, value, fee),
                        }.then(|res| {
                            if let Ok(r) = res {
                                transactions.push(r);
                            };
                            Ok(transactions) as Result<Vec<Transaction>, Error>
                        })
                    })
                }),
        )
    }

    fn create_transaction_bitcoin(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        dr_acc: Account,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let transactions_repo_clone = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let keys_client = self.keys_client.clone();
        let blockchain_client = self.blockchain_client.clone();

        Box::new(
            db_executor
                .execute_transaction(move || {
                    // creating transaction in db
                    let new_transaction: NewTransaction = NewTransaction {
                        id: TransactionId::generate(),
                        user_id,
                        dr_account_id: dr_acc.id,
                        cr_account_id: cr_acc.id,
                        currency: Currency::Btc,
                        value,
                        status: TransactionStatus::Pending,
                        blockchain_tx_id: None,
                        hold_until: None,
                    };
                    let transaction = transactions_repo
                        .create(new_transaction.clone())
                        .map_err(ectx!(try convert => new_transaction))?;

                    // sending transactions to blockchain
                    let dr_address = dr_acc.address.clone();
                    let utxos = blockchain_client
                        .get_bitcoin_utxos(dr_address.clone())
                        .map_err(ectx!(try convert => dr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input =
                        CreateBlockchainTx::new(dr_acc.address, cr_acc.address, Currency::Btc, value, fee, None, Some(utxos));

                    let raw = keys_client
                        .sign_transaction(token.clone(), create_blockchain_input.clone())
                        .map_err(ectx!(try convert => create_blockchain_input))
                        .wait()?;

                    let tx_id = blockchain_client.post_bitcoin_transaction(raw).map_err(ectx!(try convert)).wait()?;

                    Ok((transaction, tx_id))
                }).and_then(move |(transaction, tx_id)| {
                    //updating transaction with tx_id
                    let transaction_id = transaction.id;
                    db_executor
                        .execute(move || transactions_repo_clone.update_blockchain_tx(transaction_id, tx_id))
                        .then(|res| match res {
                            Ok(updated_transaction) => Ok(updated_transaction),
                            Err(e) => {
                                log_and_capture_error(e);
                                Ok(transaction)
                            }
                        })
                }),
        )
    }

    fn create_transaction_ethereum(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        dr_acc: Account,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let transactions_repo_clone = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        let keys_client = self.keys_client.clone();
        let blockchain_client = self.blockchain_client.clone();

        Box::new(
            db_executor
                .execute_transaction(move || {
                    // creating transaction in db
                    let new_transaction: NewTransaction = NewTransaction {
                        id: TransactionId::generate(),
                        user_id,
                        dr_account_id: dr_acc.id,
                        cr_account_id: cr_acc.id,
                        currency,
                        value,
                        status: TransactionStatus::Pending,
                        blockchain_tx_id: None,
                        hold_until: None,
                    };
                    let transaction = transactions_repo
                        .create(new_transaction.clone())
                        .map_err(ectx!(try convert => new_transaction))?;

                    // sending transactions to blockchain
                    let dr_address = dr_acc.address.clone();
                    let nonce = blockchain_client
                        .get_ethereum_nonce(dr_address.clone())
                        .map_err(ectx!(try convert => dr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input =
                        CreateBlockchainTx::new(dr_acc.address, cr_acc.address, currency, value, fee, Some(nonce), None);

                    let raw = keys_client
                        .sign_transaction(token.clone(), create_blockchain_input.clone())
                        .map_err(ectx!(try convert => create_blockchain_input))
                        .wait()?;

                    let tx_id = blockchain_client
                        .post_ethereum_transaction(raw)
                        .map_err(ectx!(try convert))
                        .wait()?;

                    Ok((transaction, tx_id))
                }).and_then(move |(transaction, tx_id)| {
                    //updating transaction with tx_id
                    let transaction_id = transaction.id;
                    db_executor
                        .execute(move || transactions_repo_clone.update_blockchain_tx(transaction_id, tx_id))
                        .then(|res| match res {
                            Ok(updated_transaction) => Ok(updated_transaction),
                            Err(e) => {
                                log_and_capture_error(e);
                                Ok(transaction)
                            }
                        })
                }),
        )
    }
    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<Transaction>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                let transaction = transactions_repo
                    .get(transaction_id)
                    .map_err(ectx!(try ErrorKind::Internal => transaction_id))?;
                if let Some(ref transaction) = transaction {
                    if transaction.user_id != user.id {
                        return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                    }
                }
                Ok(transaction)
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
                        .get_account_balance(account_id)
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
        offset: TransactionId,
        limit: i64,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute(move || {
                if user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }
                transactions_repo
                    .list_for_user(user_id, offset, limit)
                    .map_err(ectx!(convert => user_id, offset, limit))
            })
        }))
    }
    fn get_account_transactions(
        &self,
        token: AuthenticationToken,
        account_id: AccountId,
    ) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
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
                transactions_repo.list_for_account(account_id).map_err(ectx!(convert => account_id))
            })
        }))
    }
    fn deposit_funds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute_transaction(move || {
                        let address = input.address.clone();
                        // check that cr account exists and it is belonging to one user
                        let cr_acc = accounts_repo
                            .get_by_address(address.clone(), AccountKind::Cr)
                            .map_err(ectx!(try convert => address, AccountKind::Cr))?;
                        let cr_acc = cr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => user.id))?;
                        if cr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        let address = input.address.clone();
                        // check that dr account exists and it is belonging to one user
                        let dr_acc = accounts_repo
                            .get_by_address(address.clone(), AccountKind::Dr)
                            .map_err(ectx!(try convert => address, AccountKind::Dr))?;
                        let dr_acc = dr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => user.id))?;
                        if dr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        // creating transaction
                        let new_transaction: NewTransaction = NewTransaction::from_deposit(input, cr_acc.id, dr_acc.id);
                        transactions_repo
                            .create(new_transaction.clone())
                            .map_err(ectx!(convert => new_transaction))
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
            accounts_repo,
            db_executor,
            keys_client,
            blockchain_client,
        );
        (acc_service, trans_service)
    }

    #[test]
    fn test_transaction_create() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

;        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core
            .run(acc_service.create_account(token.clone(), user_id, cr_account.clone()))
            .unwrap();

        let mut new_transaction = CreateTransactionLocal::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.cr_account = cr_account;
        new_transaction.dr_account = dr_account;

        let transaction = core.run(trans_service.create_transaction_local(new_transaction));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_get_by_id() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;

        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.address = cr_account.address;
        new_transaction.user_id = user_id;

        let transaction = core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();
        let transaction = core.run(trans_service.get_transaction(token, transaction.id));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_get_for_users() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;

        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.address = cr_account.address;
        new_transaction.user_id = user_id;

        let transaction = core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

        let transactions = core.run(trans_service.get_transactions_for_user(token, user_id, transaction.id, 10));
        assert!(transactions.is_ok());
        assert_eq!(transactions.unwrap().len(), 1);
    }
    #[test]
    fn test_transaction_get_for_account() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = CreateTransactionLocal::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.cr_account = cr_account;
        new_transaction.dr_account = dr_account;

        let transaction = core.run(trans_service.create_transaction_local(new_transaction)).unwrap();
        let transaction = core.run(trans_service.get_account_transactions(token, transaction.cr_account_id));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_deposit_funds() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;

        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.address = cr_account.address;

        let transaction = core.run(trans_service.deposit_funds(token.clone(), new_transaction));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_withdraw() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        //creating withdraw account
        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        //depositing on withdraw account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

        //creating random account
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        //depositin on random account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = cr_account.address;

        core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

        //withdrawing
        let mut withdraw = Withdraw::default();
        withdraw.value = Amount::new(100);
        withdraw.dr_account = dr_account;

        let transaction = core.run(trans_service.withdraw(token.clone(), withdraw));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_account_get_balance() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut new_account = CreateAccount::default();
        new_account.name = "test test test acc".to_string();
        new_account.user_id = user_id;

        core.run(acc_service.create_account(token.clone(), user_id, new_account.clone()))
            .unwrap();

        let account = core.run(trans_service.get_account_balance(token, new_account.id));
        assert!(account.is_ok());
    }
}
