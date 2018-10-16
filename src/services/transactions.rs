use std::sync::Arc;

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
    fn create_transaction_local(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionLocal,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn deposit_founds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
    fn get_transaction(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
    ) -> Box<Future<Item = Option<Transaction>, Error = Error> + Send>;
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
    fn update_transaction_status(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
        transaction_status: TransactionStatus,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;

    fn create_transaction_etherium(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        cr_acc: Account,
        dr_acc: Account,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn create_transaction_bitcoin(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        cr_acc: Account,
        dr_acc: Account,
        value: Amount,
        fee: Amount,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction_local(
        &self,
        token: AuthenticationToken,
        input: CreateTransactionLocal,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
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
                        // check that cr account exists and it is belonging to one user
                        let cr_account_id = input.cr_account_id;
                        let cr_acc = accounts_repo
                            .get(cr_account_id)
                            .map_err(ectx!(try ErrorKind::Internal => cr_account_id))?;
                        let cr_acc = cr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => cr_account_id))?;
                        if cr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        // check that dr account exists and it is belonging to one user
                        let dr_account_id = input.dr_account_id;
                        let dr_acc = accounts_repo
                            .get(dr_account_id)
                            .map_err(ectx!(try ErrorKind::Internal => dr_account_id))?;
                        let dr_acc = dr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => dr_account_id))?;
                        if dr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        // check that balance > value of input
                        if cr_acc.balance > input.value {
                            let new_transaction: NewTransaction = input.into();
                            let transaction = transactions_repo
                                .create(new_transaction.clone())
                                .map_err(ectx!(try convert => new_transaction))?;
                            let value = transaction.value;
                            let cr_account_id = transaction.cr_account_id;
                            accounts_repo
                                .inc_balance(cr_account_id, value)
                                .map_err(ectx!(try convert => cr_account_id, value))?;

                            let dr_account_id = transaction.dr_account_id;
                            accounts_repo
                                .dec_balance(dr_account_id, value)
                                .map_err(ectx!(try convert => dr_account_id, value))?;
                            Ok(transaction)
                        } else {
                            Err(ectx!(err ErrorContext::NotEnoughFounds, ErrorKind::Balance => cr_acc.balance, input.value))
                        }
                    })
                })
        }))
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
    fn update_transaction_status(
        &self,
        token: AuthenticationToken,
        transaction_id: TransactionId,
        transaction_status: TransactionStatus,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(self.auth_service.authenticate(token).and_then(move |user| {
            db_executor.execute_transaction(move || {
                let transaction = transactions_repo
                    .update_status(transaction_id, transaction_status)
                    .map_err(ectx!(try convert=> transaction_id, transaction_status))?;

                if transaction.user_id != user.id {
                    return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                }

                if transaction_status == TransactionStatus::Done {
                    let value = transaction.value;
                    let cr_account_id = transaction.cr_account_id;
                    accounts_repo
                        .inc_balance(cr_account_id, value)
                        .map_err(ectx!(try convert => cr_account_id, value))?;

                    let dr_account_id = transaction.dr_account_id;
                    accounts_repo
                        .dec_balance(dr_account_id, value)
                        .map_err(ectx!(try convert => dr_account_id, value))?;
                }
                Ok(transaction)
            })
        }))
    }
    fn deposit_founds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send> {
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
                        if cr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        let address = input.address.clone();
                        // check that dr account exists and it is belonging to one user
                        let dr_acc = accounts_repo
                            .get_by_address(address.clone(), AccountKind::Dr)
                            .map_err(ectx!(try convert => address, AccountKind::Dr))?;
                        if dr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        // creating transaction
                        let new_transaction: NewTransaction = (input, cr_acc.id, dr_acc.id).into();
                        let transaction = transactions_repo
                            .create(new_transaction.clone())
                            .map_err(ectx!(try convert => new_transaction))?;
                        let value = transaction.value;
                        let cr_account_id = transaction.cr_account_id;
                        accounts_repo
                            .inc_balance(cr_account_id, value)
                            .map_err(ectx!(try convert => cr_account_id, value))?;

                        let dr_account_id = transaction.dr_account_id;
                        accounts_repo
                            .inc_balance(dr_account_id, value)
                            .map_err(ectx!(try convert => dr_account_id, value))?;
                        Ok(transaction)
                    })
                })
        }))
    }
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let currency = input.currency;
        let user_id = input.user_id;
        let fee = input.fee;
        let service = self.clone();
        Box::new(self.auth_service.authenticate(token.clone()).and_then(move |user| {
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute(move || {
                        let account_id = input.account_id;
                        // check that cr account exists and it is belonging to one user
                        let cr_acc = accounts_repo
                            .get(account_id)
                            .map_err(ectx!(try convert => account_id, AccountKind::Cr))?;
                        let cr_acc = cr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => user.id))?;
                        if cr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }
                        let currency = input.currency;
                        if cr_acc.currency != currency {
                            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Balance => currency));
                        }
                        let value = input.value;
                        if cr_acc.balance < value {
                            return Err(ectx!(err ErrorContext::NotEnoughFounds, ErrorKind::Balance => value));
                        }

                        accounts_repo
                            .get_with_enough_value(value, currency, user_id)
                            .map_err(ectx!(convert ErrorContext::NotEnoughFounds => value, currency, user_id))
                            .map(|dr_accs| (cr_acc, dr_accs))
                    })
                }).and_then(move |(cr_acc, dr_accs)| {
                    iter_ok::<_, Error>(dr_accs).fold(vec![], move |mut transactions, (dr_acc, value)| {
                        match currency {
                            Currency::Eth => service.create_transaction_etherium(
                                token.clone(),
                                user_id,
                                cr_acc.clone(),
                                dr_acc,
                                value,
                                fee,
                                Currency::Eth,
                            ),
                            Currency::Stq => service.create_transaction_etherium(
                                token.clone(),
                                user_id,
                                cr_acc.clone(),
                                dr_acc,
                                value,
                                fee,
                                Currency::Stq,
                            ),
                            Currency::Btc => service.create_transaction_bitcoin(token.clone(), user_id, cr_acc.clone(), dr_acc, value, fee),
                        }.then(|res| {
                            if let Ok(r) = res {
                                transactions.push(r);
                            };
                            Ok(transactions) as Result<Vec<Transaction>, Error>
                        })
                    })
                })
        }))
    }

    fn create_transaction_bitcoin(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        cr_acc: Account,
        dr_acc: Account,
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
                    let cr_address = cr_acc.address.clone();
                    let utxos = blockchain_client
                        .get_bitcoin_utxos(cr_address.clone())
                        .map_err(ectx!(try convert => cr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input =
                        CreateBlockchainTx::new(cr_acc.address, dr_acc.address.clone(), Currency::Btc, value, fee, None, Some(utxos));

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

    fn create_transaction_etherium(
        &self,
        token: AuthenticationToken,
        user_id: UserId,
        cr_acc: Account,
        dr_acc: Account,
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
                    let cr_address = cr_acc.address.clone();
                    let nonce = blockchain_client
                        .get_etherium_nonce(cr_address.clone())
                        .map_err(ectx!(try convert => cr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input =
                        CreateBlockchainTx::new(cr_acc.address, dr_acc.address.clone(), currency, value, fee, Some(nonce), None);

                    let raw = keys_client
                        .sign_transaction(token.clone(), create_blockchain_input.clone())
                        .map_err(ectx!(try convert => create_blockchain_input))
                        .wait()?;

                    let tx_id = blockchain_client
                        .post_etherium_transaction(raw)
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
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        let mut new_transaction = CreateTransactionLocal::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.cr_account_id = cr_account.id;
        new_transaction.dr_account_id = dr_account.id;

        let transaction = core.run(trans_service.create_transaction_local(token.clone(), new_transaction));
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

        let transaction = core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();
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

        let transaction = core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();

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
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        let mut new_transaction = CreateTransactionLocal::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.cr_account_id = cr_account.id;
        new_transaction.dr_account_id = dr_account.id;

        let transaction = core
            .run(trans_service.create_transaction_local(token.clone(), new_transaction))
            .unwrap();
        let transaction = core.run(trans_service.get_account_transactions(token, transaction.cr_account_id));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_deposit_founds() {
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

        let transaction = core.run(trans_service.deposit_founds(token.clone(), new_transaction));
        assert!(transaction.is_ok());
    }
    #[test]
    fn test_transaction_withdraw() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        //creating withdraw account
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), user_id, cr_account)).unwrap();

        //depositing on withdraw account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), deposit)).unwrap();

        //creating random account
        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), user_id, dr_account)).unwrap();

        //depositin on random account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = dr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), deposit)).unwrap();

        //withdrawing
        let mut withdraw = Withdraw::default();
        withdraw.value = Amount::new(100);
        withdraw.account_id = cr_account.id;

        let transaction = core.run(trans_service.withdraw(token.clone(), withdraw));
        assert!(transaction.is_ok());
    }
}
