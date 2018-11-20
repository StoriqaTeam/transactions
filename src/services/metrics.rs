use std::collections::HashMap;
use std::sync::Arc;

use client::BlockchainClient;
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, DbExecutor, Isolation, TransactionsRepo};

use super::error::*;

const BLOCKCHAIN_BALANCES_CONCURRENCY: usize = 20;

pub trait MetricsService: Send + Sync + 'static {
    fn get_metrics(&self) -> Box<Future<Item = Metrics, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct MetricsServiceImpl<E: DbExecutor> {
    config: Arc<Config>,
    accounts_repo: Arc<AccountsRepo>,
    transactions_repo: Arc<TransactionsRepo>,
    blockchain_client: Arc<BlockchainClient>,
    db_executor: E,
}

impl<E: DbExecutor> MetricsServiceImpl<E> {
    pub fn new(
        config: Arc<Config>,
        accounts_repo: Arc<AccountsRepo>,
        transactions_repo: Arc<TransactionsRepo>,
        db_executor: E,
        blockchain_client: Arc<BlockchainClient>,
    ) -> Self {
        MetricsServiceImpl {
            config,
            accounts_repo,
            transactions_repo,
            blockchain_client,
            db_executor,
        }
    }
}

impl<E: DbExecutor> MetricsService for MetricsServiceImpl<E> {
    fn get_metrics(&self) -> Box<Future<Item = Metrics, Error = Error> + Send> {
        let self_clone = self.clone();
        let self_2 = self.clone();
        Box::new(
            self.db_executor
                .execute_transaction_with_isolation(Isolation::RepeatableRead, move || {
                    let mut metrics: Metrics = Default::default();
                    self_clone.update_counts(&mut metrics)?;
                    let balances = self_clone.transactions_repo.get_blockchain_balances()?;
                    let reduced_balances = self_clone.update_negative_balances_and_reduce(&mut metrics, balances)?;
                    let _ = self_clone.update_fees_and_liquidity_balances(&mut metrics)?;
                    self_clone.update_limits(&mut metrics);
                    self_clone.update_total_payments_system_balances(&mut metrics, &reduced_balances);
                    Ok((metrics, reduced_balances))
                }).and_then(move |(mut metrics, reduced_balances)| {
                    let self_3 = self_2.clone();
                    self_2.fetch_blockchain_balances(&reduced_balances).map(move |blockchain_balances| {
                        self_3.update_blockchain_balances(&mut metrics, &reduced_balances, &blockchain_balances);
                        metrics
                    })
                }),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemAccountKind {
    Liquidity,
    Fee,
}

impl<E: DbExecutor> MetricsServiceImpl<E> {
    fn update_counts(&self, metrics: &mut Metrics) -> Result<(), Error> {
        let counts = self.accounts_repo.count_by_user().map_err(ectx!(try ErrorKind::Internal))?;
        let total = counts.iter().map(|(_, v)| v).sum();
        metrics.accounts_count = counts;
        metrics.accounts_count_total = total;
        Ok(())
    }

    fn update_blockchain_balances(
        &self,
        metrics: &mut Metrics,
        payments_system_balances: &HashMap<(BlockchainAddress, Currency), Amount>,
        blockchain_balances: &HashMap<(BlockchainAddress, Currency), Amount>,
    ) {
        let mut total_blockchain_balances: HashMap<Currency, f64> = HashMap::new();
        for currency in [Currency::Btc, Currency::Stq, Currency::Eth].into_iter() {
            total_blockchain_balances.insert(*currency, 0.0);
        }
        for ((_, currency), value) in blockchain_balances.iter() {
            total_blockchain_balances
                .entry(*currency)
                .and_modify(|balance| {
                    *balance += value.to_super_unit(*currency);
                }).or_insert(0.0);
        }
        metrics.total_blockchain_balances = total_blockchain_balances;

        let mut diverging_blockchain_balances: Vec<DivergingBalance> = Vec::new();
        for ((address, currency), payments_balance) in payments_system_balances {
            let blockchain_balance = blockchain_balances
                .get(&(address.clone(), *currency))
                .cloned()
                .unwrap_or(Amount::new(0));
            if blockchain_balance != *payments_balance {
                diverging_blockchain_balances.push(DivergingBalance {
                    address: address.clone(),
                    currency: *currency,
                    payments_system_value: payments_balance.to_super_unit(*currency),
                    blockchain_value: blockchain_balance.to_super_unit(*currency),
                });
            }
        }
        metrics.diverging_blockchain_balances = diverging_blockchain_balances;

        let mut diverging_blockchain_balances_total: HashMap<Currency, f64> = HashMap::new();
        for currency in [Currency::Btc, Currency::Stq, Currency::Eth].into_iter() {
            diverging_blockchain_balances_total.insert(*currency, 0.0);
        }

        for div_balance in metrics.diverging_blockchain_balances.iter() {
            diverging_blockchain_balances_total
                .entry(div_balance.currency)
                .and_modify(|value| *value += (div_balance.payments_system_value - div_balance.blockchain_value).abs());
        }

        metrics.diverging_blockchain_balances_total = diverging_blockchain_balances_total;
    }

    fn update_total_payments_system_balances(&self, metrics: &mut Metrics, balances: &HashMap<(BlockchainAddress, Currency), Amount>) {
        let mut res: HashMap<Currency, f64> = HashMap::new();
        for currency in [Currency::Btc, Currency::Stq, Currency::Eth].into_iter() {
            res.insert(*currency, 0.0);
        }
        for ((_, currency), value) in balances.iter() {
            res.entry(*currency)
                .and_modify(|balance| {
                    *balance += value.to_super_unit(*currency);
                }).or_insert(0.0);
        }
        metrics.total_payments_system_balances = res;
    }

    fn fetch_blockchain_balances(
        &self,
        balances: &HashMap<(BlockchainAddress, Currency), Amount>,
    ) -> impl Future<Item = HashMap<(BlockchainAddress, Currency), Amount>, Error = Error> {
        let self_ = self.clone();
        let keys: Vec<_> = balances.keys().cloned().collect();
        let stream = futures::stream::iter_ok(keys)
            .map(move |(address, currency)| {
                let address_ = address.clone();
                let address_2 = address.clone();
                self_
                    .blockchain_client
                    .get_balance(address.clone(), currency)
                    .map(move |value| ((address_, currency), value))
                    .map_err(ectx!(ErrorKind::Internal => address_2))
            }).buffered(BLOCKCHAIN_BALANCES_CONCURRENCY);
        stream.collect().map(|vec| {
            let res: HashMap<(BlockchainAddress, Currency), Amount> = vec.into_iter().collect();
            res
        })
    }

    fn update_fees_and_liquidity_balances(&self, metrics: &mut Metrics) -> Result<(), Error> {
        let balances = self.transactions_repo.get_system_balances()?;
        let mut liquidity_balances: HashMap<Currency, f64> = HashMap::new();
        let mut fees_balances: HashMap<Currency, f64> = HashMap::new();
        for currency in [Currency::Btc, Currency::Stq, Currency::Eth].into_iter() {
            liquidity_balances.insert(
                *currency,
                self.extract_balance(SystemAccountKind::Liquidity, *currency, metrics, &balances)?,
            );
            fees_balances.insert(
                *currency,
                self.extract_balance(SystemAccountKind::Fee, *currency, metrics, &balances)?,
            );
        }
        metrics.fees_balances = fees_balances;
        metrics.liquidity_balances = liquidity_balances;
        Ok(())
    }

    fn extract_balance(
        &self,
        kind: SystemAccountKind,
        currency: Currency,
        metrics: &mut Metrics,
        balances: &HashMap<AccountId, (Amount, Amount)>,
    ) -> Result<f64, Error> {
        let account_id = match (kind, currency) {
            (SystemAccountKind::Fee, Currency::Btc) => self.config.system.btc_fees_account_id,
            (SystemAccountKind::Fee, Currency::Eth) => self.config.system.eth_fees_account_id,
            (SystemAccountKind::Fee, Currency::Stq) => self.config.system.stq_fees_account_id,
            (SystemAccountKind::Liquidity, Currency::Btc) => self.config.system.btc_liquidity_account_id,
            (SystemAccountKind::Liquidity, Currency::Eth) => self.config.system.eth_liquidity_account_id,
            (SystemAccountKind::Liquidity, Currency::Stq) => self.config.system.stq_liquidity_account_id,
        };
        let balance_pair = balances.get(&account_id).cloned().unwrap_or((Amount::new(0), Amount::new(0)));
        match balance_pair.0.checked_sub(balance_pair.1) {
            Some(balance) => Ok(balance.to_super_unit(currency)),
            None => {
                let account = self
                    .accounts_repo
                    .get(account_id)?
                    .ok_or(ectx!(try err ErrorContext::NoAccount, ErrorKind::NotFound))?;
                if metrics
                    .negative_balances
                    .iter()
                    .find(|neg_balance| (neg_balance.address == account.address) && (neg_balance.currency == account.currency))
                    .is_none()
                {
                    metrics.negative_balances.push(NegativeBalance {
                        address: account.address,
                        currency: account.currency,
                        value: balance_pair.1.checked_sub(balance_pair.0).unwrap(),
                    });
                }
                Ok(0.0)
            }
        }
    }

    fn update_limits(&self, metrics: &mut Metrics) {
        let mut limits: HashMap<Currency, f64> = HashMap::new();
        limits.insert(Currency::Btc, self.config.limits.btc_limit);
        limits.insert(Currency::Eth, self.config.limits.eth_limit);
        limits.insert(Currency::Stq, self.config.limits.stq_limit);
        metrics.limits = limits;
    }

    fn update_negative_balances_and_reduce(
        &self,
        metrics: &mut Metrics,
        balances: HashMap<(BlockchainAddress, Currency), (Amount, Amount)>,
    ) -> Result<HashMap<(BlockchainAddress, Currency), Amount>, Error> {
        let mut neg_res: Vec<NegativeBalance> = Vec::new();
        let mut res: HashMap<(BlockchainAddress, Currency), Amount> = HashMap::new();
        for key in balances.keys() {
            let (dr_turnover, cr_turnover) = balances[key];
            if cr_turnover > dr_turnover {
                neg_res.push(NegativeBalance {
                    address: key.0.clone(),
                    currency: key.1,
                    value: cr_turnover.checked_sub(dr_turnover).unwrap(),
                });
            } else {
                res.insert(key.clone(), dr_turnover.checked_sub(cr_turnover).unwrap());
            }
        }
        metrics.negative_balances = neg_res;
        Ok(res)
    }
}
