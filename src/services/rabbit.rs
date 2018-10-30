use std::sync::Arc;

use super::error::*;
use models::*;
use prelude::*;
use repos::error::{Error as RepoError, ErrorContext as RepoErrorContex, ErrorKind as RepoErrorKind};
use repos::{
    AccountsRepo, BlockchainTransactionsRepo, DbExecutor, PendingBlockchainTransactionsRepo, SeenHashesRepo,
    StrangeBlockchainTransactionsRepo, TransactionsRepo,
};
use serde_json;
use utils::log_error;

pub const ETHERIUM_PRICE: u128 = 200; // 200$, price of 1 eth in gwei
pub const STQ_PRICE: f64 = 0.0025; // 0,0025$, price of 1 stq in gwei
pub const WEI: u128 = 1_000_000_000_000_000_000;
pub const BITCOIN_PRICE: u128 = 6400; // 6400$ price in satoshi
pub const SATOSHI: u128 = 100_000_000;

#[derive(Clone)]
pub struct BlockchainFetcher<E: DbExecutor> {
    transactions_repo: Arc<TransactionsRepo>,
    accounts_repo: Arc<AccountsRepo>,
    seen_hashes_repo: Arc<SeenHashesRepo>,
    blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
    strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn new(
        transactions_repo: Arc<TransactionsRepo>,
        accounts_repo: Arc<AccountsRepo>,
        seen_hashes_repo: Arc<SeenHashesRepo>,
        blockchain_transactions_repo: Arc<BlockchainTransactionsRepo>,
        strange_blockchain_transactions_repo: Arc<StrangeBlockchainTransactionsRepo>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        db_executor: E,
    ) -> Self {
        BlockchainFetcher {
            transactions_repo,
            accounts_repo,
            seen_hashes_repo,
            blockchain_transactions_repo,
            strange_blockchain_transactions_repo,
            pending_blockchain_transactions_repo,
            db_executor,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
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
    #[fail(display = "blockchain transaction invariant violation - withdrawal blockchain tx value is not equal to pending tx value")]
    DepositAddressInternal,
}

impl<E: DbExecutor> BlockchainFetcher<E> {
    pub fn handle_message(&self, data: Vec<u8>) -> impl Future<Item = (), Error = Error> + Send {
        let db_executor = self.db_executor.clone();
        let self_clone = self.clone();
        parse_transaction(data)
            .into_future()
            .and_then(move |tx| db_executor.execute_transaction(move || self_clone.handle_transaction(&tx)))
    }

    fn handle_transaction(&self, blockchain_tx: &BlockchainTransaction) -> Result<(), Error> {
        let normalized_tx = blockchain_tx
            .normalized()
            .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal))?;
        // already processed this transaction - skipping
        if let Ok(_) = self.seen_hashes_repo.get(normalized_tx.hash.clone(), normalized_tx.currency) {
            return Ok(());
        }

        if let Some(tx) = self.transactions_repo.get_by_blockchain_tx(normalized_tx.hash.clone())? {
            // The tx is already in our db => it was created by us and waiting for confirmation from blockchain => it's withdrawal tx
            let total_tx_value = normalized_tx
                .value()
                .ok_or(ectx!(try err ErrorContext::BalanceOverflow, ErrorKind::Internal => tx.clone()))?;
            if required_confirmations(normalized_tx.currency, total_tx_value) > normalized_tx.confirmations as u64 {
                // skipping tx, waiting for more confirms
                return Ok(());
            }

            if let Some(violation) = self.verify_withdrawal_tx(&tx, &normalized_tx)? {
                // Here the tx itself is ok, but violates our internal invariants. We just log it here and put it into strange blockchain transactions table
                // If we instead returned error - it would nack the rabbit message and return it to queue - smth we don't want here
                self.handle_violation(violation, &blockchain_tx)?;
                return Ok(());
            }
            self.blockchain_transactions_repo.create(blockchain_tx.clone().into())?;
            self.pending_blockchain_transactions_repo.delete(blockchain_tx.hash.clone())?;
            self.transactions_repo
                .update_status(blockchain_tx.hash.clone(), TransactionStatus::Done)?;
            self.seen_hashes_repo.create(NewSeenHashes {
                hash: blockchain_tx.hash.clone(),
                block_number: blockchain_tx.block_number as i64,
                currency: blockchain_tx.currency,
            })?;
            return Ok(());
        };

        let to_addresses: Vec<_> = normalized_tx.to.iter().map(|entry| entry.address.clone()).collect();
        let matched_dr_accounts = self
            .accounts_repo
            .get_by_addresses(&to_addresses, blockchain_tx.currency, AccountKind::Dr)?;
        if matched_dr_accounts.len() == 0 {
            self.seen_hashes_repo.create(NewSeenHashes {
                hash: blockchain_tx.hash.clone(),
                block_number: blockchain_tx.block_number as i64,
                currency: blockchain_tx.currency,
            })?;
            return Ok(());
        }

        if let Some(violation) = self.verify_deposit_tx(&normalized_tx)? {
            self.handle_violation(violation, &blockchain_tx)?;
            return Ok(());
        }

        for to_dr_account in matched_dr_accounts {
            let to_entry = blockchain_tx
                .to
                .iter()
                .find(|entry| entry.address == to_dr_account.address)
                .unwrap();
            let to_cr_account = self
                .accounts_repo
                .get_by_address(to_dr_account.address, to_dr_account.currency, AccountKind::Cr)?
                .unwrap();
            let new_tx = NewTransaction {
                id: TransactionId::generate(),
                user_id: to_dr_account.user_id,
                dr_account_id: to_dr_account.id,
                cr_account_id: to_cr_account.id,
                currency: to_dr_account.currency,
                value: to_entry.value,
                status: TransactionStatus::Done,
                blockchain_tx_id: Some(blockchain_tx.hash.clone()),
                hold_until: None,
                fee: blockchain_tx.fee,
            };
            self.transactions_repo.create(new_tx)?;
            self.blockchain_transactions_repo.create(blockchain_tx.clone().into())?;
            self.seen_hashes_repo.create(NewSeenHashes {
                hash: blockchain_tx.hash.clone(),
                block_number: blockchain_tx.block_number as i64,
                currency: blockchain_tx.currency,
            })?;
        }
        Ok(())
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

    pub fn process(&self, data: Vec<u8>) -> impl Future<Item = (), Error = Error> {
        let data_clone = data.clone();
        let transactions_repo = self.transactions_repo.clone();
        let seen_hashes_repo = self.seen_hashes_repo.clone();
        let accounts_repo = self.accounts_repo.clone();
        let blockchain_transactions_repo = self.blockchain_transactions_repo.clone();
        let strange_blockchain_transactions_repo = self.strange_blockchain_transactions_repo.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let db_executor = self.db_executor.clone();
        Box::new(
            String::from_utf8(data.clone())
                .map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => data_clone))
                .into_future()
                .and_then(|s| serde_json::from_str(&s).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => s)))
                .and_then(move |blockchain_transaction: BlockchainTransaction| {
                    db_executor
                        .execute_transaction(move || -> Result<(), RepoError> {
                            let total_value = blockchain_transaction.to.iter().try_fold(Amount::default(),|acc, x| {
                                acc.checked_add(x.value).ok_or_else(|| ectx!(err RepoErrorContex::BalanceOverflow, RepoErrorKind::Internal => x.value)) as Result<Amount, RepoError>
                            })?;
                            let enough_confirms = match blockchain_transaction.currency {
                                // # Ethereum
                                // < $20 / $200 - 0 conf
                                // < $50 / $200 - 1 conf
                                // < $200 / $200 - 2 conf
                                // < $500 / $200 - 3 conf
                                // < $1000 / $200 - 4 conf
                                // < $2000 / $200 - 5 conf
                                // < $3000 / $200 - 6 conf
                                // < $5000 / $200 - 8 conf
                                // > $5000 / $200 - 12 conf
                                Currency::Stq => match total_value.raw() {
                                    x if x < (((20f64 / STQ_PRICE) as u128) * WEI) => 0,
                                    x if x < (((50f64 / STQ_PRICE) as u128) * WEI) => 1,
                                    x if x < (((200f64 / STQ_PRICE) as u128) * WEI) => 2,
                                    x if x < (((500f64 / STQ_PRICE) as u128) * WEI) => 3,
                                    x if x < (((1000f64 / STQ_PRICE) as u128) * WEI) => 4,
                                    x if x < (((2000f64 / STQ_PRICE) as u128) * WEI) => 5,
                                    x if x < (((3000f64 / STQ_PRICE) as u128) * WEI) => 6,
                                    x if x < (((5000f64 / STQ_PRICE) as u128) * WEI) => 8,
                                    _ => 12,
                                },
                                // # Ethereum
                                // < $20 / $200 - 0 conf
                                // < $50 / $200 - 1 conf
                                // < $200 / $200 - 2 conf
                                // < $500 / $200 - 3 conf
                                // < $1000 / $200 - 4 conf
                                // < $2000 / $200 - 5 conf
                                // < $3000 / $200 - 6 conf
                                // < $5000 / $200 - 8 conf
                                // > $5000 / $200 - 12 conf
                                Currency::Eth => match total_value.raw() {
                                    x if x < (20 * WEI / ETHERIUM_PRICE) => 0,
                                    x if x < (50 * WEI / ETHERIUM_PRICE) => 1,
                                    x if x < (200 * WEI / ETHERIUM_PRICE) => 2,
                                    x if x < (500 * WEI / ETHERIUM_PRICE) => 3,
                                    x if x < (1000 * WEI / ETHERIUM_PRICE) => 4,
                                    x if x < (2000 * WEI / ETHERIUM_PRICE) => 5,
                                    x if x < (3000 * WEI / ETHERIUM_PRICE) => 6,
                                    x if x < (5000 * WEI / ETHERIUM_PRICE) => 8,
                                    _ => 12,
                                },
                                // # Bitcoin
                                // < $100 / $6400 - 0 conf
                                // < $500 / $6400 - 1 conf
                                // < $1000 / $6400 - 2 conf
                                // > $1000 / $6400 - 3 conf
                                Currency::Btc => match total_value.raw() {
                                    x if x < (100 * SATOSHI / BITCOIN_PRICE) => 0,
                                    x if x < (500 * SATOSHI / BITCOIN_PRICE) => 1,
                                    x if x < (1000 * SATOSHI / BITCOIN_PRICE) => 2,
                                    _ => 3,
                                },
                            };

                            //checking for enough confirmations
                            if blockchain_transaction.confirmations < enough_confirms {
                                return Ok(());
                            }

                            //checking blockchain hash already seen
                            if seen_hashes_repo
                                .get(blockchain_transaction.hash.clone(), blockchain_transaction.currency)?
                                .is_some()
                            {
                                return Ok(());
                            }

                            // unifying from and to
                            let (from, to) = blockchain_transaction.unify_from_to().map_err(ectx!(try convert))?;

                            let unified_hash = blockchain_transaction.hash.clone();
                            // withdraw
                            if let Some(transaction) = transactions_repo.get_by_blockchain_tx(unified_hash.clone())? {
                                // checking that `from` account exists in accounts but no `to` in accounts
                                let mut to_not_exists = true;
                                for (address, _) in to {
                                    to_not_exists &= accounts_repo
                                        .get_by_address(address.clone(), blockchain_transaction.currency, AccountKind::Cr)?
                                        .is_none()
                                }

                                if accounts_repo.get(transaction.dr_account_id)?.is_none() {
                                    let comment = format!("Withdraw transaction dr account {} does not exists.", transaction.dr_account_id);
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else if !to_not_exists {
                                    let comment = "Withdraw transaction contains our account in `to` field.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else if transaction.status == TransactionStatus::Done {
                                    let comment = "Withdraw transaction is already in done state.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                } else {
                                    transactions_repo.update_status(unified_hash.clone(), TransactionStatus::Done)?;
                                    pending_blockchain_transactions_repo.delete(unified_hash)?;
                                    blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                                }
                            } else {
                                // checking that `from` accounts not exist
                                let mut from_not_exists = true;
                                for address in from {
                                    from_not_exists &= accounts_repo
                                        .get_by_address(address.clone(), blockchain_transaction.currency, AccountKind::Cr)?
                                        .is_none()
                                }
                                if from_not_exists {
                                    // deposit
                                    for (blockchain_transaction_to, blockchain_transaction_value) in to {
                                        let mut added_transactions = false;
                                        if let Some(cr_account) = accounts_repo.get_by_address(
                                            blockchain_transaction_to.clone(),
                                            blockchain_transaction.currency,
                                            AccountKind::Cr,
                                        )? {
                                            if let Some(dr_account) = accounts_repo.get_by_address(
                                                blockchain_transaction_to.clone(),
                                                blockchain_transaction.currency,
                                                AccountKind::Dr,
                                            )? {
                                                let new_transaction = NewTransaction {
                                                    id: TransactionId::generate(),
                                                    user_id: cr_account.user_id,
                                                    dr_account_id: dr_account.id,
                                                    cr_account_id: cr_account.id,
                                                    currency: blockchain_transaction.currency,
                                                    value: blockchain_transaction_value,
                                                    status: TransactionStatus::Done,
                                                    blockchain_tx_id: Some(blockchain_transaction.hash.clone()),
                                                    hold_until: None,
                                                    fee: blockchain_transaction.fee,
                                                };
                                                transactions_repo.create(new_transaction)?;
                                                added_transactions = true;
                                            } else {
                                                return Err(
                                                    ectx!(err RepoErrorContex::AccountsPair, RepoErrorKind::Internal => blockchain_transaction_to.clone()),
                                                );
                                            }
                                        }
                                        if added_transactions {
                                            //adding blockchain transaction to db
                                            blockchain_transactions_repo.create(blockchain_transaction.clone().into())?;
                                        }
                                    }
                                } else {
                                    let comment = "Withdraw transaction hash does not exists, but `from` field contains our account.".to_string();
                                    let new_strange = (blockchain_transaction.clone(), comment).into();
                                    strange_blockchain_transactions_repo.create(new_strange)?;
                                }

                            }
                            //adding blockchain hash to already seen
                            seen_hashes_repo.create(blockchain_transaction.clone().into())?;
                            Ok(())
                        }).map_err(ectx!(ErrorSource::Repo, ErrorKind::Internal))
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
