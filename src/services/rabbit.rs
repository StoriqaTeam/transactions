use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use futures::future::{self, Either};

use super::error::*;
use super::system::{SystemService, SystemServiceImpl};
use super::transactions::converter::{ConverterService, ConverterServiceImpl};
use client::{BlockchainClient, KeysClient};
use config::Config;
use models::*;
use prelude::*;
use rabbit::TransactionPublisher;
use repos::{
    AccountsRepo, BlockchainTransactionsRepo, DbExecutor, Isolation, KeyValuesRepo, PendingBlockchainTransactionsRepo, SeenHashesRepo,
    StrangeBlockchainTransactionsRepo, TransactionsRepo,
};
use serde_json;
use utils::{log_and_capture_error, log_error};

// it's ok to have this low approval threshold, the attack is still not
// feasible, as an attacker need to spend at least 20000 gas per stq transfer
// to a new account => his cost will be smth like 30% of ours
const STQ_BALANCE_THRESHOLD: u128 = 1;
// 100 bn of storiqa
const STQ_ALLOWANCE: u128 = 100_000_000_000_000_000_000_000_000_000;

#[derive(Clone)]
pub struct BlockchainFetcher<E: DbExecutor> {
    config: Arc<Config>,
    transactions_repo: Arc<TransactionsRepo>,
    accounts_repo: Arc<AccountsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    key_values_repo: Arc<KeyValuesRepo>,
    system_service: Arc<SystemService>,
    converter_service: Arc<ConverterService>,
    blockchain_client: Arc<BlockchainClient>,
    keys_client: Arc<KeysClient>,
    db_executor: E,
    publisher: Arc<dyn TransactionPublisher>,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn new(
        config: Arc<Config>,
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<AccountsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        key_values_repo: Arc<KeyValuesRepo>,
        blockchain_client: Arc<BlockchainClient>,
        keys_client: Arc<KeysClient>,
        db_executor: E,
        publisher: Arc<dyn TransactionPublisher>,
    ) -> Self {
        let system_service = Arc::new(SystemServiceImpl::new(accounts_repo.clone(), config.clone()));
        let converter_service = Arc::new(ConverterServiceImpl::new(
            accounts_repo.clone(),
            pending_blockchain_transactions_repo.clone(),
            blockchain_transactions_repo.clone(),
            system_service.clone(),
        ));
        BlockchainFetcher {
            config,
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
            strange_blockchain_transactions_repo,
            pending_blockchain_transactions_repo,
            key_values_repo,
            system_service,
            converter_service,
            blockchain_client,
            keys_client,
            db_executor,
            publisher,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
#[allow(dead_code)]
pub enum InvariantViolation {
    #[fail(display = "blockchain transaction invariant violation - unexpected number of addresses")]
    WithdrawalAdressesCount,
    #[fail(display = "blockchain transaction invariant violation - address of transaction not found in our database")]
    WithdrawalAdressesNotFound,
    #[fail(display = "blockchain transaction invariant violation - transaction referred to non-existing account")]
    NotExistingAccount,
    #[fail(display = "blockchain transaction invariant violation - withdrawal happened to internal account, which shouldn't be the case")]
    WithdrawalAdressesInternal,
    #[fail(
        display = "blockchain transaction invariant violation - withdrawal transaction should be in pending state when blockchain tx arrives"
    )]
    WithdrawalNotPendingAddress,
    #[fail(display = "blockchain transaction invariant violation - withdrawal blockchain tx doesn't have corresponding pending part")]
    WithdrawalNoPendingTx,
    #[fail(display = "blockchain transaction invariant violation - withdrawal blockchain tx value is not equal to pending tx value")]
    WithdrawalValue,
    #[fail(display = "blockchain transaction invariant violation - deposit arrived from internal address")]
    DepositAddressInternal,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn handle_message(&self, data: Vec<u8>) -> impl Future<Item = (), Error = Error> + Send {
        let db_executor = self.db_executor.clone();
        let converter = self.converter_service.clone();
        let publisher = self.publisher.clone();
        let self_clone = self.clone();
        parse_transaction(data)
            .into_future()
            .and_then(move |tx| self_clone.handle_transaction(&tx))
            .and_then(move |txs| {
                if !txs.is_empty() {
                    info!("Sending txs: {:?}", txs);
                    Either::A(
                        db_executor
                            .execute(move || converter.convert_transaction(txs))
                            .and_then(move |tx_out| {
                                info!("Sending tx after conversion: {:?}", tx_out);
                                publisher
                                    .publish(tx_out.clone())
                                    .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal => tx_out))
                                    .then(|r: Result<(), Error>| match r {
                                        Err(e) => {
                                            log_error(&e);
                                            Ok(())
                                        }
                                        Ok(_) => Ok(()),
                                    })
                            }),
                    )
                } else {
                    Either::B(future::ok(()))
                }
            })
    }

    fn handle_transaction(&self, blockchain_tx: &BlockchainTransaction) -> impl Future<Item = Vec<Transaction>, Error = Error> + Send {
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let transactions_repo = self.transactions_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let seen_hashes_repo = self.seen_hashes_repo.clone();
        let system_service = self.system_service.clone();
        let blockchain_tx = blockchain_tx.clone();
        db_executor
            .execute_transaction_with_isolation(Isolation::Serializable, move || {
                let normalized_tx = blockchain_tx
                    .normalized()
                    .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => blockchain_tx))?;
                // already processed this transaction - skipping
                if let Some(_) = seen_hashes_repo.get(normalized_tx.hash.clone(), normalized_tx.currency)? {
                    return Ok((vec![], vec![]));
                }

                if let Some(erc20_op) = blockchain_tx.erc20_operation_kind {
                    if erc20_op == Erc20OperationKind::Approve {
                        // skip confirmations, because the value is very large,
                        // but since it's `approve` operation we don't care
                        if blockchain_tx.currency != Currency::Stq {
                            return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal => blockchain_tx));
                        }
                        let from = blockchain_tx
                    .from
                    .get(0)
                    .ok_or(
                        ectx!(try err ErrorContext::InvalidBlockchainTransactionStructure, ErrorKind::Internal => blockchain_tx.clone()),
                    )?
                    .clone();
                        if let Some(account) = accounts_repo.get_by_address(from.clone(), Currency::Stq, AccountKind::Dr)? {
                            if !account.erc20_approved {
                                let changeset = UpdateAccount {
                                    erc20_approved: Some(true),
                                    ..Default::default()
                                };
                                accounts_repo.update(account.id, changeset.clone())?;
                                // We don't need the notion of approved credit account anymore, as all debit accounts get approved
                                blockchain_transactions_repo.create(blockchain_tx.clone().into())?;
                                pending_blockchain_transactions_repo.delete(blockchain_tx.hash.clone())?;
                                seen_hashes_repo.create(NewSeenHashes {
                                    hash: blockchain_tx.hash.clone(),
                                    block_number: blockchain_tx.block_number as i64,
                                    currency: blockchain_tx.currency,
                                })?;
                            }
                        }
                        // don't need to collect fees, etc. - see the comment in that send_erc20_approval
                        return Ok((vec![], vec![]));
                    }
                }

                if let Some(tx) = transactions_repo.get_by_blockchain_tx(normalized_tx.hash.clone())? {
                    // The tx is already in our db => it was created by us and waiting for confirmation from blockchain => it's withdrawal tx
                    let total_tx_value = normalized_tx
                        .value()
                        .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => tx.clone()))?;
                    if required_confirmations(normalized_tx.currency, total_tx_value) > normalized_tx.confirmations as u64 {
                        // skipping tx, waiting for more confirms
                        return Ok((vec![], vec![]));
                    }
                    if let Some(violation) = self_clone.verify_withdrawal_tx(&tx, &normalized_tx)? {
                        // Here the tx itself is ok, but violates our internal invariants. We just log it here and put it into strange blockchain transactions table
                        // If we instead returned error - it would nack the rabbit message and return it to queue - smth we don't want here
                        self_clone.handle_violation(violation, &blockchain_tx)?;
                        return Ok((vec![], vec![]));
                    }
                    let fees_currency = match blockchain_tx.currency {
                        Currency::Btc => Currency::Btc,
                        Currency::Eth => Currency::Eth,
                        Currency::Stq => Currency::Eth,
                    };

                    let fees_account_dr = match blockchain_tx.currency {
                        // stq accounts bear eth fees, that are written off from system account
                        Currency::Stq => system_service.get_system_fees_account_dr(fees_currency)?,
                        // other accounts make withdrawal from some dr account, which is stored in tx.cr_account_id
                        // and fees will be written off from them
                        _ => accounts_repo
                            .get(tx.cr_account_id)?
                            .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => blockchain_tx, fees_currency))?,
                    };
                    let fees_account_cr = system_service.get_system_fees_account(fees_currency)?;
                    blockchain_transactions_repo.create(blockchain_tx.clone().into())?;
                    pending_blockchain_transactions_repo.delete(blockchain_tx.hash.clone())?;
                    transactions_repo.update_status(blockchain_tx.hash.clone(), TransactionStatus::Done)?;
                    let fee_tx = NewTransaction {
                        id: TransactionId::generate(),
                        gid: tx.gid,
                        user_id: tx.user_id,
                        dr_account_id: fees_account_cr.id,
                        cr_account_id: fees_account_dr.id,
                        currency: fees_currency,
                        value: blockchain_tx.fee,
                        status: TransactionStatus::Done,
                        blockchain_tx_id: None,
                        kind: TransactionKind::BlockchainFee,
                        group_kind: tx.group_kind,
                        related_tx: None,
                        meta: None,
                    };
                    transactions_repo.create(fee_tx)?;
                    seen_hashes_repo.create(NewSeenHashes {
                        hash: blockchain_tx.hash.clone(),
                        block_number: blockchain_tx.block_number as i64,
                        currency: blockchain_tx.currency,
                    })?;
                    return Ok((vec![], vec![]));
                };

                let to_addresses: Vec<_> = normalized_tx.to.iter().map(|entry| entry.address.clone()).collect();
                let matched_dr_accounts = accounts_repo.get_by_addresses(&to_addresses, blockchain_tx.currency, AccountKind::Dr)?;
                if matched_dr_accounts.len() == 0 {
                    seen_hashes_repo.create(NewSeenHashes {
                        hash: blockchain_tx.hash.clone(),
                        block_number: blockchain_tx.block_number as i64,
                        currency: blockchain_tx.currency,
                    })?;
                    return Ok((vec![], vec![]));
                }

                if let Some(violation) = self_clone.verify_deposit_tx(&normalized_tx)? {
                    self_clone.handle_violation(violation, &blockchain_tx)?;
                    return Ok((vec![], vec![]));
                }

                let mut transactions_out = vec![];
                let mut need_approve = vec![];

                let mut idx = 0;
                for to_dr_account in matched_dr_accounts {
                    let Account {
                        address: to_dr_address,
                        currency: to_dr_currency,
                        ..
                    } = to_dr_account.clone();
                    let to_entry = blockchain_tx
                        .to
                        .iter()
                        .find(|entry| entry.address == to_dr_address.clone())
                        .ok_or(ectx!(try err ErrorContext::MissingAddressInTx, ErrorKind::Internal => to_dr_address.clone()))?;
                    let to_cr_account = accounts_repo
                        .get_by_address(to_dr_address.clone(), to_dr_currency.clone(), AccountKind::Cr)?
                        .ok_or(
                            ectx!(try err ErrorContext::NoAccount, ErrorKind::Internal => to_dr_address, to_dr_currency, AccountKind::Cr),
                        )?;
                    let tx_id = TransactionId::generate();
                    let new_tx = NewTransaction {
                        id: tx_id,
                        gid: tx_id,
                        user_id: to_dr_account.user_id,
                        dr_account_id: to_dr_account.id,
                        cr_account_id: to_cr_account.id,
                        currency: to_dr_account.currency,
                        value: to_entry.value,
                        status: TransactionStatus::Done,
                        blockchain_tx_id: Some(blockchain_tx.hash.clone()),
                        kind: TransactionKind::Deposit,
                        group_kind: TransactionGroupKind::Deposit,
                        related_tx: None,
                        meta: None,
                    };
                    let dr_transaction = transactions_repo.create(new_tx)?;
                    transactions_out.push(dr_transaction);
                    // don't need to create these more than one time, or conflict will be o/w
                    if idx == 0 {
                        blockchain_transactions_repo.create(blockchain_tx.clone().into())?;
                        seen_hashes_repo.create(NewSeenHashes {
                            hash: blockchain_tx.hash.clone(),
                            block_number: blockchain_tx.block_number as i64,
                            currency: blockchain_tx.currency,
                        })?;
                    };
                    // approve account if balance has passed threshold
                    if (to_dr_account.currency == Currency::Stq) && !to_dr_account.erc20_approved {
                        let balance = transactions_repo.get_accounts_balance(to_dr_account.user_id, &[to_dr_account.clone()])?[0].balance;
                        if balance >= Amount::new(STQ_BALANCE_THRESHOLD) {
                            need_approve.push(to_dr_account)
                        }
                    }
                    idx += 1;
                }
                Ok((transactions_out, need_approve))
            })
            .and_then(move |(transactions_out, need_approve)| {
                let self_clone2 = self_clone2.clone();
                futures::stream::iter_ok(need_approve)
                    .for_each(move |to_dr_account| self_clone2.clone().send_erc20_approval(&to_dr_account))
                    .then(move |res| {
                        if let Err(e) = res {
                            log_and_capture_error(e);
                        }
                        Ok(transactions_out)
                    })
            })
    }

    fn handle_violation(&self, violation: InvariantViolation, blockchain_tx: &BlockchainTransaction) -> Result<(), Error> {
        log_error(&ectx!(try err violation => blockchain_tx));

        let message = format!("{}", violation);
        let new_strange_tx = (blockchain_tx.clone(), message).into();
        self.strange_blockchain_transactions_repo.create(new_strange_tx)?;

        self.seen_hashes_repo.create(NewSeenHashes {
            hash: blockchain_tx.hash.clone(),
            block_number: blockchain_tx.block_number as i64,
            currency: blockchain_tx.currency,
        })?;
        Ok(())
    }

    fn verify_deposit_tx(&self, blockchain_tx: &BlockchainTransaction) -> Result<Option<InvariantViolation>, Error> {
        let from_accounts = self
            .accounts_repo
            .get_by_addresses(&blockchain_tx.from, blockchain_tx.currency, AccountKind::Dr)?;
        if from_accounts.len() > 0 {
            return Ok(Some(InvariantViolation::DepositAddressInternal));
        }
        Ok(None)
    }

    // Returns error if there's an error in connecting to db, etc. (in this case it makes sense to nack and retry after)
    // Returns Ok(None) if the transaction is ok
    // Returns Ok(Some(violation)) if some invariants are broken (in this case, transaction is permanently broken, so write it
    // to strange transactions and ack)
    //
    // Withdrawal tx is in form:
    //
    // | dr_acc_id                | cr_acc_id                                                      |   |
    // |--------------------------|----------------------------------------------------------------|---|
    // | User's account (Cr type) | Our internal acc with blockchain money managed by us (Dr type) |   |
    fn verify_withdrawal_tx(&self, tx: &Transaction, blockchain_tx: &BlockchainTransaction) -> Result<Option<InvariantViolation>, Error> {
        // Our withdrawal transactions are 1 to 1.
        if (blockchain_tx.from.len() != 1) || (blockchain_tx.to.len() != 1) {
            return Ok(Some(InvariantViolation::WithdrawalAdressesCount));
        }
        if tx.status != TransactionStatus::Pending {
            return Ok(Some(InvariantViolation::WithdrawalNotPendingAddress));
        }
        if self.pending_blockchain_transactions_repo.get(blockchain_tx.hash.clone())?.is_none() {
            return Ok(Some(InvariantViolation::WithdrawalNoPendingTx));
        }

        let from_address = blockchain_tx.from[0].clone();
        let BlockchainTransactionEntryTo { address: to_address, .. } = blockchain_tx.to[0].clone();
        // Transaction should have valid account in our db
        if let Some(managed_address) = self.accounts_repo.get(tx.cr_account_id)? {
            // Blockchain tx from_address should be equal to that of manages account address
            if managed_address.address != from_address {
                return Ok(Some(InvariantViolation::WithdrawalAdressesNotFound));
            }
        } else {
            return Ok(Some(InvariantViolation::NotExistingAccount));
        };
        // to_address should be external to our system, because in all other cases we should do
        // everything internally
        if let Some(_) = self
            .accounts_repo
            .get_by_address(to_address.clone(), blockchain_tx.currency, AccountKind::Dr)?
        {
            return Ok(Some(InvariantViolation::WithdrawalAdressesInternal));
        }
        // to_address should be external to our system, because in all other cases we should do
        // everything internally
        if let Some(_) = self
            .accounts_repo
            .get_by_address(to_address.clone(), blockchain_tx.currency, AccountKind::Cr)?
        {
            return Ok(Some(InvariantViolation::WithdrawalAdressesInternal));
        }
        // values in blockchain and our tx must match
        // TODO - subject to fees
        // if value != tx.value {
        //     return Ok(Some(InvariantViolation::WithdrawalValue));
        // }
        Ok(None)
    }

    fn send_erc20_approval(&self, account: &Account) -> Box<Future<Item = (), Error = Error> + Send> {
        let account = account.clone();
        let account_address = account.address.clone();
        let approve_gas_price = self.config.system.approve_gas_price;
        let approve_gas_limit = self.config.system.approve_gas_limit;
        let db_executor = self.db_executor.clone();
        let db_executor_clone = self.db_executor.clone();
        let db_executor_clone2 = self.db_executor.clone();
        let blockchain_client = self.blockchain_client.clone();
        let blockchain_client_ = self.blockchain_client.clone();
        let blockchain_client__ = self.blockchain_client.clone();
        let blockchain_client___ = self.blockchain_client.clone();
        let system_service = self.system_service.clone();
        let system_service_ = self.system_service.clone();
        let key_values_repo = self.key_values_repo.clone();
        let keys_client = self.keys_client.clone();
        let keys_client_ = self.keys_client.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let pending_blockchain_transactions_repo_ = self.pending_blockchain_transactions_repo.clone();
        let transactions_repo = self.transactions_repo.clone();
        let approve_delay_secs = self.config.system.approve_delay_secs;

        Box::new(
            db_executor
                .execute(move || {
                    system_service
                        .get_system_fees_account_dr(Currency::Eth)
                        .map(|eth_fees_dr_account| (eth_fees_dr_account.address, eth_fees_dr_account.id))
                })
                .and_then(move |(tx_initiator, tx_initiator_id)| {
                    let tx_initiator_ = tx_initiator.clone();
                    blockchain_client
                        .get_ethereum_nonce(tx_initiator.clone())
                        .map_err(ectx!(convert => tx_initiator_))
                        .map(move |ethereum_nonce| (ethereum_nonce, tx_initiator, tx_initiator_id))
                })
                .and_then(move |(ethereum_nonce, tx_initiator, tx_initiator_id)| {
                    db_executor.execute(move || {
                        let tx_initiator_ = tx_initiator.clone();
                        let maybe_db_nonce = key_values_repo
                            .get_nonce(tx_initiator_.clone())
                            .map_err(ectx!(try ErrorKind::Internal))?;
                        let nonce = match (maybe_db_nonce, ethereum_nonce) {
                            (None, ethereum_nonce) => ethereum_nonce,
                            (Some(db_nonce), ethereum_nonce) => {
                                // if db nonce was updated more than a minute ago
                                // and it is not equal to blockchain nonce we use blockchain value
                                if Utc::now().naive_utc() - db_nonce.updated_at > ChronoDuration::seconds(60) {
                                    key_values_repo
                                        .set_nonce(tx_initiator.clone(), ethereum_nonce)
                                        .map_err(ectx!(try ErrorKind::Internal))?;
                                    ethereum_nonce
                                } else {
                                    // if for some reason we missed blockchain nonce (for example, new transaction was send wright before)
                                    db_nonce.value.as_u64().unwrap_or_default().max(ethereum_nonce)
                                }
                            }
                        };
                        let _ = key_values_repo
                            .set_nonce(tx_initiator.clone(), nonce + 1)
                            .map_err(ectx!(try ErrorKind::Internal => tx_initiator, nonce + 1))?;
                        Ok((nonce, tx_initiator_, tx_initiator_id))
                    })
                })
                .and_then(move |(eth_fees_account_nonce, tx_initiator, tx_initiator_id)| {
                    let id = TransactionId::generate();
                    let next_id = id.next();
                    Amount::new(approve_gas_price as u128)
                        .checked_mul(Amount::new(approve_gas_limit as u128))
                        .ok_or(ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal))
                        .into_future()
                        .and_then(move |approve_value| {
                            let eth_transfer_blockchain_tx = CreateBlockchainTx {
                                id,
                                from: tx_initiator.clone(),
                                to: account.address.clone(),
                                currency: Currency::Eth,
                                value: approve_value,
                                fee_price: approve_gas_price,
                                nonce: Some(eth_fees_account_nonce),
                                utxos: None,
                            };

                            // TODO: sign_transaction will use transferFrom, meaning
                            // you have to approve it for self before that.
                            // Need to add ordinary transfer method
                            let eth_transfer_blockchain_tx_clone = eth_transfer_blockchain_tx.clone();
                            keys_client
                                .sign_transaction(eth_transfer_blockchain_tx.clone(), Role::System)
                                .map_err(move |e| ectx!(err e, ErrorKind::Internal => eth_transfer_blockchain_tx.clone()))
                                .and_then(move |eth_raw_tx| {
                                    blockchain_client_
                                        .post_ethereum_transaction(eth_raw_tx.clone())
                                        .map_err(move |e| ectx!(err e, ErrorKind::Internal => eth_raw_tx))
                                })
                                .and_then(move |eth_tx_id| {
                                    db_executor_clone.execute(move || {
                                        let eth_fees_cr_account = system_service_.get_system_fees_account(Currency::Eth)?;
                                        let eth_tx = NewTransaction {
                                            id,
                                            gid: id,
                                            user_id: account.user_id,
                                            dr_account_id: eth_fees_cr_account.id,
                                            cr_account_id: tx_initiator_id,
                                            currency: Currency::Eth,
                                            value: approve_value,
                                            status: TransactionStatus::Pending,
                                            blockchain_tx_id: Some(eth_tx_id.clone()),
                                            kind: TransactionKind::ApprovalTransfer,
                                            group_kind: TransactionGroupKind::Approval,
                                            related_tx: None,
                                            meta: None,
                                        };
                                        let new_pending_eth = (eth_transfer_blockchain_tx_clone, eth_tx_id.clone()).into();
                                        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
                                        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
                                        // it later.
                                        match pending_blockchain_transactions_repo.create(new_pending_eth) {
                                            Err(e) => log_and_capture_error(e),
                                            _ => (),
                                        };
                                        transactions_repo.create(eth_tx)?;
                                        Ok((next_id, tx_initiator))
                                    })
                                })
                        })
                })
                .and_then(move |(next_id, tx_initiator)| {
                    // next step - we send approve operation after some delay
                    // we don't wait for it though, because o/w it will block database
                    // connection for 1 min or smth like this.

                    // the other tricky thing is we store pending_blockchain_tx for tracking status,
                    // but we don't create transctions. Why? Because we already spent some ether into
                    // non-trackable eth account (having the same address as our in-system stq account).
                    // Therefore we will make approval and spend that fee off-system.

                    // So the total fee will be ether transfer (needed for approve call) + fee of this
                    // transfer
                    let account_address_clone = account_address.clone();

                    blockchain_client__
                        .get_ethereum_nonce(account_address.clone())
                        .map_err(ectx!(ErrorKind::Internal => account_address_clone))
                        .and_then(move |approve_nonce| {
                            let eth_approve_blockchain_tx = ApproveInput {
                                id: next_id,
                                address: account_address.clone(),
                                approve_address: tx_initiator.clone(),
                                currency: Currency::Stq,
                                value: Amount::new(STQ_ALLOWANCE),
                                fee_price: approve_gas_price,
                                nonce: approve_nonce,
                            };
                            let eth_approve_blockchain_tx_clone2 = eth_approve_blockchain_tx.clone();

                            let when = Instant::now() + Duration::from_secs(approve_delay_secs);
                            tokio::timer::Delay::new(when)
                                .map_err(ectx!(ErrorContext::Timer, ErrorKind::Internal))
                                .and_then(move |_| {
                                    keys_client_
                                        .approve(eth_approve_blockchain_tx.clone(), Role::User)
                                        .map_err(ectx!(ErrorKind::Internal => eth_approve_blockchain_tx))
                                })
                                .and_then(move |approve_raw_tx| {
                                    blockchain_client___
                                        .post_ethereum_transaction(approve_raw_tx.clone())
                                        .map_err(ectx!(ErrorKind::Internal => approve_raw_tx))
                                })
                                .and_then(move |approve_tx_id| {
                                    // logs from blockchain gw erc20 comes with log number in hash
                                    let approve_tx_id = BlockchainTransactionId::new(format!("{}:0", approve_tx_id.inner()));
                                    let new_pending_approve = (eth_approve_blockchain_tx_clone2, approve_tx_id.clone()).into();
                                    db_executor_clone2.execute(move || -> Result<(), Error> {
                                        match pending_blockchain_transactions_repo_.create(new_pending_approve) {
                                            Err(e) => log_and_capture_error(e),
                                            _ => (),
                                        };
                                        Ok(())
                                    })
                                })
                        })
                }),
        )
    }
}

const USD_PER_ETH: f64 = 200.0;
const USD_PER_BTC: f64 = 6500.0;
const USD_PER_STQ: f64 = 0.0025;
const BTC_DECIMALS: u128 = 100_000_000u128;
const ETH_DECIMALS: u128 = 1_000_000_000_000_000_000u128;
const STQ_DECIMALS: u128 = 1_000_000_000_000_000_000u128;
const BTC_CONFIRM_THRESHOLDS: &[u64] = &[100, 500, 1000];
const ETH_CONFIRM_THRESHOLDS: &[u64] = &[20, 50, 200, 500, 1000, 2000, 3000, 4000, 5000];

fn to_usd_approx(currency: Currency, value: Amount) -> u64 {
    let (rate, decimals) = match currency {
        Currency::Btc => (USD_PER_BTC, BTC_DECIMALS),
        Currency::Eth => (USD_PER_ETH, ETH_DECIMALS),
        Currency::Stq => (USD_PER_STQ, STQ_DECIMALS),
    };
    // Max of all rates
    let max_rate = USD_PER_BTC as u128;
    // first multiply by max_rate and then divide by it
    // that is made so that we can use integer division of u128 (f64 is not enough)
    // and be sure that our error is less that 1 dollar
    let crypto_value_times_rate: u128 = value.raw() * max_rate / decimals;
    // after dividing by decimals we have value small enough to be used as f64
    let usd_value: f64 = (crypto_value_times_rate as f64) * rate / (max_rate as f64);
    usd_value as u64
}

fn required_confirmations(currency: Currency, value: Amount) -> u64 {
    let usd_value = to_usd_approx(currency, value);
    let thresholds = match currency {
        Currency::Btc => BTC_CONFIRM_THRESHOLDS,
        _ => ETH_CONFIRM_THRESHOLDS,
    };
    let mut res = None;
    for (i, threshold) in thresholds.iter().enumerate() {
        if *threshold >= usd_value {
            res = Some(i as u64);
            break;
        }
    }
    res.unwrap_or(thresholds.len() as u64)
}

fn parse_transaction(data: Vec<u8>) -> Result<BlockchainTransaction, Error> {
    let data_clone = data.clone();
    let string = String::from_utf8(data).map_err(|e| ectx!(try err e, ErrorContext::UTF8, ErrorKind::Internal => data_clone))?;
    serde_json::from_str(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_required_confirmations() {
        let cases = [
            (Currency::Btc, Amount::new(100_000_000), 3),                       // 6500
            (Currency::Btc, Amount::new(10_000_000), 2),                        // 650
            (Currency::Btc, Amount::new(5_000_000), 1),                         // 325
            (Currency::Btc, Amount::new(1_000_000), 0),                         // 65
            (Currency::Eth, Amount::new(21_000_000_000_000_000_000), 8),        // 4400
            (Currency::Eth, Amount::new(2_000_000_000_000_000_000), 3),         // 400
            (Currency::Eth, Amount::new(500_000_000_000_000_000), 2),           // 100
            (Currency::Eth, Amount::new(50_000_000_000_000_000), 0),            // 10
            (Currency::Stq, Amount::new(2_100_000_000_000_000_000_000_000), 9), // 5250
            (Currency::Stq, Amount::new(210_000_000_000_000_000_000_000), 4),   // 525
            (Currency::Stq, Amount::new(100_000_000_000_000_000_000_000), 3),   // 250
            (Currency::Stq, Amount::new(10_000_000_000_000_000_000_000), 1),    // 25
            (Currency::Stq, Amount::new(5_000_000_000_000_000_000_000), 0),     // 12
        ];
        for (currency, value, confirms) in cases.iter() {
            assert_eq!(
                required_confirmations(*currency, *value),
                *confirms,
                "Currency: {:?}, value: {:?}, confirms: {:?}",
                *currency,
                *value,
                *confirms
            );
        }
    }
}
