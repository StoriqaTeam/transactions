use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use future::Either;
use futures::IntoFuture;

use super::super::error::*;
use super::super::system::SystemService;
use client::{BlockchainClient, ExchangeClient, KeysClient};
use config::Config;
use models::*;
use prelude::*;
use repos::{DbExecutor, KeyValuesRepo, PendingBlockchainTransactionsRepo};
use utils::log_and_capture_error;

pub struct FeeEstimate {
    pub gross_fee: Amount,
    pub fee_price: f64,
    pub currency: Currency,
}

pub trait BlockchainService: Send + Sync + 'static {
    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error>>;
    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
        currency: Currency,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error>>;
    fn estimate_withdrawal_fee(
        &self,
        input_gross_fee: Amount,
        input_fee_currency: Currency,
        withdrawal_currency: Currency,
    ) -> Box<Future<Item = FeeEstimate, Error = Error>>;
}

#[derive(Clone)]
pub struct BlockchainServiceImpl<E: DbExecutor> {
    config: Arc<Config>,
    keys_client: Arc<dyn KeysClient>,
    blockchain_client: Arc<dyn BlockchainClient>,
    exchange_client: Arc<dyn ExchangeClient>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    key_values_repo: Arc<KeyValuesRepo>,
    system_service: Arc<SystemService>,
    db_executor: E,
}

impl<E: DbExecutor> BlockchainServiceImpl<E> {
    pub fn new(
        config: Arc<Config>,
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
        exchange_client: Arc<ExchangeClient>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
        key_values_repo: Arc<KeyValuesRepo>,
        system_service: Arc<SystemService>,
        db_executor: E,
    ) -> Self {
        Self {
            config,
            keys_client,
            blockchain_client,
            exchange_client,
            pending_blockchain_transactions_repo,
            key_values_repo,
            system_service,
            db_executor,
        }
    }
}

impl<E: DbExecutor> BlockchainService for BlockchainServiceImpl<E> {
    // Note that withdrawal_currency may not equal to FeeEstimate currency. E.g. if
    // withdrawal_currency = stq, FeeEstimate currency will = eth (since you need eth to withdraw stq)
    fn estimate_withdrawal_fee(
        &self,
        input_gross_fee: Amount,
        input_fee_currency: Currency,
        withdrawal_currency: Currency,
    ) -> Box<Future<Item = FeeEstimate, Error = Error>> {
        let estimate_currency = match withdrawal_currency {
            Currency::Btc => Currency::Btc,
            Currency::Eth => Currency::Eth,
            Currency::Stq => Currency::Eth,
        };
        let base = match withdrawal_currency {
            Currency::Btc => self.config.fees_options.btc_transaction_size,
            Currency::Eth => self.config.fees_options.eth_gas_limit,
            Currency::Stq => self.config.fees_options.stq_gas_limit,
        };
        let base = Amount::new(base as u128);
        let exchange_client = self.exchange_client.clone();
        Box::new(
            input_gross_fee
                .checked_div(Amount::new(self.config.fees_options.fee_upside as u128))
                .ok_or(ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal))
                .into_future()
                .and_then(move |total_blockchain_fee_native_currency| {
                    if input_fee_currency == estimate_currency {
                        Either::A(futures::future::ok(total_blockchain_fee_native_currency))
                    } else {
                        let input_rate = RateInput {
                            id: ExchangeId::generate(),
                            from: input_fee_currency,
                            to: estimate_currency,
                            amount: total_blockchain_fee_native_currency,
                            amount_currency: input_fee_currency,
                        };
                        Either::B(
                            exchange_client
                                .rate(input_rate.clone(), Role::System)
                                .map_err(ectx!(ErrorKind::Internal => input_rate))
                                .map(|Rate { rate, .. }| {
                                    total_blockchain_fee_native_currency.convert(input_fee_currency, estimate_currency, rate)
                                }),
                        )
                    }
                })
                .and_then(move |total_blockchain_fee_esitmate_currency| {
                    total_blockchain_fee_esitmate_currency
                        .checked_div(base)
                        .ok_or(ectx!(err ErrorContext::BalanceOverflow, ErrorKind::Internal))
                        .map(|fee_price_int| {
                            // Because of bad accuracy of u128 division in low bits
                            // we use f64 division if result is lower then 1000
                            let fee_price = if fee_price_int < Amount::new(1000) {
                                (total_blockchain_fee_esitmate_currency.raw() as f64) / (base.raw() as f64)
                            } else {
                                fee_price_int.raw() as f64
                            };
                            FeeEstimate {
                                gross_fee: total_blockchain_fee_esitmate_currency,
                                fee_price,
                                currency: estimate_currency,
                            }
                        })
                }),
        )
    }

    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error>> {
        let from_clone = from.clone();
        let db_executor = self.db_executor.clone();
        let blockchain_client = self.blockchain_client.clone();
        let keys_client = self.keys_client.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        Box::new(
            self.blockchain_client
                .get_bitcoin_utxos(from.clone())
                .map_err(ectx!(convert => from_clone))
                .and_then(move |utxos| {
                    let create_blockchain_input = CreateBlockchainTx::new(from, to, Currency::Btc, value, fee_price, None, Some(utxos));
                    let create_blockchain_input_clone = create_blockchain_input.clone();

                    keys_client
                        .sign_transaction(create_blockchain_input.clone(), Role::User)
                        .map_err(ectx!(convert => create_blockchain_input_clone, Role::User))
                        .and_then(move |raw_tx| {
                            blockchain_client
                                .post_bitcoin_transaction(raw_tx.clone())
                                .map_err(ectx!(convert => raw_tx))
                        })
                        .and_then(move |blockchain_tx_id| {
                            db_executor.execute(move || {
                                let new_pending = (create_blockchain_input, blockchain_tx_id.clone()).into();
                                // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
                                // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
                                // it later.
                                match pending_blockchain_transactions_repo.create(new_pending) {
                                    Err(e) => log_and_capture_error(e),
                                    _ => (),
                                };

                                Ok(blockchain_tx_id)
                            })
                        })
                }),
        )
    }

    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee_price: f64,
        currency: Currency,
    ) -> Box<Future<Item = BlockchainTransactionId, Error = Error>> {
        let db_executor = self.db_executor.clone();
        let blockchain_client = self.blockchain_client.clone();
        let keys_client = self.keys_client.clone();
        let pending_blockchain_transactions_repo = self.pending_blockchain_transactions_repo.clone();
        let key_values_repo = self.key_values_repo.clone();
        let system_service = self.system_service.clone();

        match currency {
            Currency::Eth => (),
            Currency::Stq => (),
            _ => {
                return Box::new(futures::future::err(
                    ectx!(err ErrorContext::InvalidCurrency, ErrorKind::InvalidInput(currency.to_string())),
                ));
            }
        };
        Box::new(
            db_executor
                .execute(move || match currency {
                    Currency::Stq => system_service
                        .get_system_fees_account(Currency::Eth)
                        .map_err(ectx!(ErrorKind::Internal => Currency::Eth))
                        .map(|account| account.address),
                    _ => Ok(from),
                })
                .and_then(move |tx_initiator| {
                    let tx_initiator_ = tx_initiator.clone();
                    blockchain_client
                        .get_ethereum_nonce(tx_initiator.clone())
                        .map_err(ectx!(convert => tx_initiator_))
                        .map(|ethereum_nonce| (ethereum_nonce, tx_initiator))
                })
                .and_then(move |(ethereum_nonce, tx_initiator)| {
                    db_executor.execute(move || {
                        let tx_initiator_ = tx_initiator.clone();
                        let maybe_db_nonce = match currency {
                            Currency::Stq | Currency::Eth => key_values_repo
                                .get_nonce(tx_initiator_.clone())
                                .map_err(ectx!(try ErrorKind::Internal))?,
                            _ => None,
                        };
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

                        // TODO, at this stage transaction is dropped if there's another tx in progress
                        // but this needs to be additionally verified
                        // Therefore we don't do any ether transaction
                        // alternative - use locks, but there are also caveats depending on the transactions isolation
                        // and master / slave reads
                        // https://www.postgresql.org/docs/9.6/applevel-consistency.html
                        // https://www.postgresql.org/docs/9.6/explicit-locking.html

                        // sleeping so there's a guaranteed interval between withdrawals
                        std::thread::sleep(std::time::Duration::from_millis(1500));

                        Ok(nonce)
                    })
                })
                .and_then(move |nonce| {
                    // creating blockchain transactions array
                    let create_blockchain_input = CreateBlockchainTx::new(from, to, currency, value, fee_price, Some(nonce), None);

                    let create_blockchain = create_blockchain_input.clone();

                    keys_client
                        .sign_transaction(create_blockchain_input.clone(), Role::User)
                        .map_err(ectx!(convert => create_blockchain_input))
                        .and_then(|raw_tx| {
                            blockchain_client
                                .post_ethereum_transaction(raw_tx.clone())
                                .map_err(ectx!(convert => raw_tx))
                        })
                        .and_then(move |tx_id| {
                            db_executor.execute(move || {
                                let tx_id = match currency {
                                    Currency::Eth => tx_id,
                                    // Erc-20 token, we need event log number here, to make a tx_id unique
                                    _ => BlockchainTransactionId::new(format!("{}:0", tx_id)),
                                };
                                let new_pending = (create_blockchain, tx_id.clone()).into();
                                // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
                                // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
                                // it later.
                                match pending_blockchain_transactions_repo.create(new_pending) {
                                    Err(e) => log_and_capture_error(e),
                                    _ => (),
                                };
                                Ok(tx_id)
                            })
                        })
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use client::*;
    use config::Config;
    use repos::*;
    use services::*;
    use tokio_core::reactor::Core;

    fn create_blockchain_service() -> BlockchainServiceImpl<DbExecutorMock> {
        let config = Arc::new(Config::new().unwrap());
        let keys_client = Arc::new(KeysClientMock::default());
        let blockchain_client = Arc::new(BlockchainClientMock::default());
        let exchange_client = Arc::new(ExchangeClientMock::default());
        let pending_blockchain_transactions_repo = Arc::new(PendingBlockchainTransactionsRepoMock::default());
        let key_values_repo = Arc::new(KeyValuesRepoMock::default());
        let transfer_accounts: [Account; 3] = [Account::default(), Account::default(), Account::default()];
        let liquidity_accounts: [Account; 3] = [Account::default(), Account::default(), Account::default()];
        let fees_accounts: [Account; 3] = [Account::default(), Account::default(), Account::default()];
        let fees_accounts_dr: [Account; 3] = [Account::default(), Account::default(), Account::default()];
        let system_service = Arc::new(SystemServiceMock::new(
            transfer_accounts,
            liquidity_accounts,
            fees_accounts,
            fees_accounts_dr,
        ));
        let db_executor = DbExecutorMock::default();
        BlockchainServiceImpl::new(
            config,
            keys_client,
            blockchain_client,
            exchange_client,
            pending_blockchain_transactions_repo,
            key_values_repo,
            system_service,
            db_executor,
        )
    }

    #[test]
    fn test_blockchain_create_btc_happy() {
        let service = create_blockchain_service();
        let mut core = Core::new().unwrap();
        let res = core.run(service.create_bitcoin_tx(BlockchainAddress::default(), BlockchainAddress::default(), Amount::new(0), 0f64));
        assert!(res.is_ok());
        let res = core.run(service.create_bitcoin_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(100500),
            0f64,
        ));
        assert!(res.is_ok());
        let res = core.run(service.create_bitcoin_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            100500f64,
        ));
        assert!(res.is_ok());
        let res = core.run(service.create_bitcoin_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(1005000),
            100500f64,
        ));
        assert!(res.is_ok());
    }

    #[test]
    fn test_blockchain_create_eth_happy() {
        let mut core = Core::new().unwrap();
        let service = create_blockchain_service();
        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            0f64,
            Currency::Eth,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(100500000000),
            0f64,
            Currency::Eth,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            100500f64,
            Currency::Eth,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(100500),
            100500f64,
            Currency::Eth,
        ));
        assert!(res.is_ok());
    }

    #[test]
    fn test_blockchain_create_stq_happy() {
        let mut core = Core::new().unwrap();
        let service = create_blockchain_service();
        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            0f64,
            Currency::Stq,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(100500000000),
            0f64,
            Currency::Stq,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            100500f64,
            Currency::Stq,
        ));
        assert!(res.is_ok());

        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(100500),
            100500f64,
            Currency::Stq,
        ));
        assert!(res.is_ok());
    }

    #[test]
    fn test_blockchain_create_estimate_withdrawal_fee_happy() {
        let mut core = Core::new().unwrap();
        let service = create_blockchain_service();
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Stq, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Stq, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Stq, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Stq, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Stq, Currency::Btc));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Stq, Currency::Btc));
        assert!(res.is_ok());

        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Eth, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Eth, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Eth, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Eth, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Eth, Currency::Btc));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Eth, Currency::Btc));
        assert!(res.is_ok());

        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Btc, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Btc, Currency::Stq));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Btc, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Btc, Currency::Eth));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(0), Currency::Btc, Currency::Btc));
        assert!(res.is_ok());
        let res = core.run(service.estimate_withdrawal_fee(Amount::new(100500000), Currency::Btc, Currency::Btc));
        assert!(res.is_ok());
    }

    #[test]
    fn test_blockchain_create_eth_wrong_currency() {
        let mut core = Core::new().unwrap();
        let service = create_blockchain_service();
        let res = core.run(service.create_ethereum_tx(
            BlockchainAddress::default(),
            BlockchainAddress::default(),
            Amount::new(0),
            0f64,
            Currency::Btc,
        ));
        assert!(res.is_err());
    }
}
