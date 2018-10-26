use std::sync::Arc;

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
use repos::{AccountsRepo, BlockchainTransactionsRepo, DbExecutor, PendingBlockchainTransactionsRepo, TransactionsRepo};
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
}

pub trait TransactionsService: Send + Sync + 'static {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransaction,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send>;
    fn create_transaction_local(&self, input: CreateTransactionLocal) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn deposit_funds(&self, token: AuthenticationToken, input: DepositFounds) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn withdraw(&self, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send>;
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
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send>;
    fn create_transaction_ethereum(
        &self,
        user_id: UserId,
        dr_acc: AccountId,
        address: AccountAddress,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn create_transaction_bitcoin(
        &self,
        user_id: UserId,
        dr_acc: AccountId,
        address: AccountAddress,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send>;
    fn convert_transaction(&self, transaction: Transaction) -> Box<Future<Item = TransactionOut, Error = Error> + Send>;
}

impl<E: DbExecutor> TransactionsService for TransactionsServiceImpl<E> {
    fn create_transaction(
        &self,
        token: AuthenticationToken,
        input: CreateTransaction,
    ) -> Box<Future<Item = Vec<TransactionOut>, Error = Error> + Send> {
        let accounts_repo = self.accounts_repo.clone();
        let db_executor = self.db_executor.clone();
        let service = self.clone();
        let service2 = self.clone();
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
                            let currency = input.to_currency;
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
                                        .get_by_address(cr_account_address.clone(), currency, AccountKind::Cr)
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
                        Either::B(service.withdraw(Withdraw::new(&input, dr_acc, cr_account_address)))
                    }
                }).and_then(move |transactions| {
                    iter_ok::<_, Error>(transactions).fold(vec![], move |mut transactions, transaction| {
                        service2.convert_transaction(transaction).and_then(|res| {
                            transactions.push(res);
                            Ok(transactions) as Result<Vec<TransactionOut>, Error>
                        })
                    })
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
    fn withdraw(&self, input: Withdraw) -> Box<Future<Item = Vec<Transaction>, Error = Error> + Send> {
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

                        let txs = transactions_repo
                            .get_with_enough_value(value, currency, user_id)
                            .map_err(ectx!(try convert ErrorContext::NotEnoughFounds => value, currency, user_id))?;

                        //double check
                        for tx in &txs {
                            let tx_id = tx.0.id;
                            let needed_amount = tx.1;
                            let balance = transactions_repo.get_account_balance(tx_id).map_err(ectx!(try convert => tx_id))?;
                            if balance < needed_amount {
                                return Err(ectx!(err ErrorContext::NotEnoughFounds, ErrorKind::Balance => balance, needed_amount));
                            }
                        }
                        Ok((input.dr_account.id, input.address, txs))
                    })
                }).and_then(move |(dr_acc_id, address, cr_accs)| {
                    iter_ok::<_, Error>(cr_accs).fold(vec![], move |mut transactions, (cr_acc, value)| {
                        match currency {
                            Currency::Eth => {
                                service.create_transaction_ethereum(user_id, dr_acc_id, address.clone(), cr_acc, value, fee, Currency::Eth)
                            }
                            Currency::Stq => {
                                service.create_transaction_ethereum(user_id, dr_acc_id, address.clone(), cr_acc, value, fee, Currency::Stq)
                            }
                            Currency::Btc => service.create_transaction_bitcoin(user_id, dr_acc_id, address.clone(), cr_acc, value, fee),
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
        user_id: UserId,
        dr_acc_id: AccountId,
        address: AccountAddress,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let pending_transactions_repo = self.pending_transactions_repo.clone();
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
                        dr_account_id: dr_acc_id,
                        cr_account_id: cr_acc.id,
                        currency: Currency::Btc,
                        value,
                        status: TransactionStatus::Pending,
                        blockchain_tx_id: None,
                        hold_until: None,
                        fee,
                    };
                    let transaction = transactions_repo
                        .create(new_transaction.clone())
                        .map_err(ectx!(try convert => new_transaction))?;

                    // sending transactions to blockchain
                    let dr_address = address.clone();
                    let utxos = blockchain_client
                        .get_bitcoin_utxos(dr_address.clone())
                        .map_err(ectx!(try convert => dr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input =
                        CreateBlockchainTx::new(address, cr_acc.address, Currency::Btc, value, fee, None, Some(utxos));

                    let create_blockchain = create_blockchain_input.clone();
                    let raw = keys_client
                        .sign_transaction(create_blockchain_input.clone())
                        .map_err(ectx!(try convert => create_blockchain_input))
                        .wait()?;

                    let tx_id = blockchain_client.post_bitcoin_transaction(raw).map_err(ectx!(try convert)).wait()?;

                    let new_pending = (create_blockchain, tx_id.clone()).into();
                    let _ = pending_transactions_repo.create(new_pending).map_err(|e| {
                        log_and_capture_error(e);
                    });

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
        user_id: UserId,
        dr_acc_id: AccountId,
        address: AccountAddress,
        cr_acc: Account,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Box<Future<Item = Transaction, Error = Error> + Send> {
        let transactions_repo = self.transactions_repo.clone();
        let pending_transactions_repo = self.pending_transactions_repo.clone();
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
                        dr_account_id: dr_acc_id,
                        cr_account_id: cr_acc.id,
                        currency,
                        value,
                        status: TransactionStatus::Pending,
                        blockchain_tx_id: None,
                        hold_until: None,
                        fee,
                    };
                    let transaction = transactions_repo
                        .create(new_transaction.clone())
                        .map_err(ectx!(try convert => new_transaction))?;

                    // sending transactions to blockchain
                    let dr_address = address.clone();
                    let nonce = blockchain_client
                        .get_ethereum_nonce(dr_address.clone())
                        .map_err(ectx!(try convert => dr_address))
                        .wait()?;

                    // creating blockchain transactions array
                    let create_blockchain_input = CreateBlockchainTx::new(address, cr_acc.address, currency, value, fee, Some(nonce), None);

                    let create_blockchain = create_blockchain_input.clone();
                    let raw = keys_client
                        .sign_transaction(create_blockchain_input.clone())
                        .map_err(ectx!(try convert => create_blockchain_input))
                        .wait()?;

                    let tx_id = blockchain_client
                        .post_ethereum_transaction(raw)
                        .map_err(ectx!(try convert))
                        .wait()?;

                    let new_pending = (create_blockchain, tx_id.clone()).into();
                    let _ = pending_transactions_repo.create(new_pending).map_err(|e| {
                        log_and_capture_error(e);
                    });

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
                    transactions_repo.list_for_account(account_id).map_err(ectx!(convert => account_id))
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
                        let currency = input.currency;
                        // check that cr account exists and it is belonging to one user
                        let cr_acc = accounts_repo
                            .get_by_address(address.clone(), currency, AccountKind::Cr)
                            .map_err(ectx!(try convert => address, AccountKind::Cr))?;
                        let cr_acc = cr_acc.ok_or_else(|| ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound => user.id))?;
                        if cr_acc.user_id != user.id {
                            return Err(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized => user.id));
                        }

                        let address = input.address.clone();
                        // check that dr account exists and it is belonging to one user
                        let dr_acc = accounts_repo
                            .get_by_address(address.clone(), currency, AccountKind::Dr)
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

    #[test]
    fn test_transaction_create() {
        let mut core = Core::new().unwrap();
        let token = AuthenticationToken::default();
        let user_id = UserId::generate();
        let (acc_service, trans_service) = create_services(token.clone(), user_id);

        let mut dr_account = CreateAccount::default();
        dr_account.name = "test test test acc".to_string();
        dr_account.user_id = user_id;
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

;        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account.clone())).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100500);
        new_transaction.address = cr_account.address;
        new_transaction.user_id = user_id;

        let _ = core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

        let transactions = core.run(trans_service.get_transactions_for_user(token, user_id, 0, 10));
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
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

        let mut new_transaction = DepositFounds::default();
        new_transaction.value = Amount::new(100501);
        new_transaction.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), new_transaction)).unwrap();

        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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

        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

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
        let dr_account = core.run(acc_service.create_account(token.clone(), dr_account)).unwrap();

        //depositing on withdraw account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = dr_account.address.clone();

        core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

        //creating random account
        let mut cr_account = CreateAccount::default();
        cr_account.name = "test test test acc".to_string();
        cr_account.user_id = user_id;
        let cr_account = core.run(acc_service.create_account(token.clone(), cr_account)).unwrap();

        //depositin on random account
        let mut deposit = DepositFounds::default();
        deposit.value = Amount::new(100500);
        deposit.address = cr_account.address;

        core.run(trans_service.deposit_funds(token.clone(), deposit)).unwrap();

        //withdrawing
        let mut withdraw = Withdraw::default();
        withdraw.value = Amount::new(100);
        withdraw.dr_account = dr_account;

        let transaction = core.run(trans_service.withdraw(withdraw));
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

        core.run(acc_service.create_account(token.clone(), new_account.clone())).unwrap();

        let account = core.run(trans_service.get_account_balance(token, new_account.id));
        assert!(account.is_ok());
    }
}
