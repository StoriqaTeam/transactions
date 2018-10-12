use std::sync::Arc;

use futures::IntoFuture;
use validator::Validate;

use super::auth::AuthService;
use super::error::*;
use client::KeysClient;
use models::*;
use prelude::*;
use repos::{AccountsRepo, DbExecutor, TransactionsRepo};

#[derive(Clone)]
pub struct TransactionsServiceImpl<E: DbExecutor> {
    auth_service: Arc<dyn AuthService>,
    transactions_repo: Arc<dyn TransactionsRepo>,
    accounts_repo: Arc<dyn AccountsRepo>,
    db_executor: E,
    keys_client: Arc<dyn KeysClient>,
}

impl<E: DbExecutor> TransactionsServiceImpl<E> {
    pub fn new(
        auth_service: Arc<AuthService>,
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<dyn AccountsRepo>,
        db_executor: E,
        keys_client: Arc<dyn KeysClient>,
    ) -> Self {
        Self {
            auth_service,
            transactions_repo,
            accounts_repo,
            db_executor,
            keys_client,
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
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Transaction, Error = Error> + Send>;
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
    fn withdraw(&self, token: AuthenticationToken, input: Withdraw) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let keys_client = self.keys_client.clone();
        Box::new(self.auth_service.authenticate(token.clone()).and_then(move |user| {
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then(move |_| {
                    db_executor.execute_transaction(move || {
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

                        let value = input.value;
                        let currency = input.currency;
                        let user_id = input.user_id;
                        // find acc with min enough balance
                        let dr_acc = accounts_repo
                            .get_min_enough_value(value, currency, user_id)
                            .map_err(ectx!(try convert ErrorContext::NotEnoughFounds => value, currency, user_id))?;

                        // creating transaction in blockchain
                        let create_blockchain_input = CreateBlockchainTx::new(cr_acc.address, dr_acc.address, input.value, input.currency);

                        let blockchain_tx_id = keys_client
                            .create_blockchain_tx(token, create_blockchain_input.clone())
                            .map_err(ectx!(try convert => create_blockchain_input))
                            .wait()?;

                        // creating transaction in db
                        let new_transaction: NewTransaction = (input, dr_acc.id, blockchain_tx_id).into();
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
        let db_executor = DbExecutorMock::default();
        let acc_service = AccountsServiceImpl::new(
            auth_service.clone(),
            accounts_repo.clone(),
            db_executor.clone(),
            keys_client.clone(),
        );
        let trans_service = TransactionsServiceImpl::new(auth_service, transactions_repo, accounts_repo, db_executor, keys_client);
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
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), new_transaction)).unwrap();

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

        //depositing on withdraw account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = cr_account.address;

        core.run(trans_service.deposit_founds(token.clone(), deposit)).unwrap();

        //creating random account
        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

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
