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
        Box::new(
            self.db_executor
                .execute_transaction_with_isolation(Isolation::RepeatableRead, move || {
                    let mut metrics: Metrics = Default::default();
                    self_clone.update_counts(&mut metrics)?;
                    let balances = self_clone.transactions_repo.get_blockchain_balances()?;
                    let _reduced_balances = self_clone.update_negative_balances_and_reduce(&mut metrics, balances)?;
                    Ok(metrics)
                }),
        )
    }
}

impl<E: DbExecutor> MetricsServiceImpl<E> {
    fn update_counts(&self, metrics: &mut Metrics) -> Result<(), Error> {
        let counts = self.accounts_repo.count_by_user().map_err(ectx!(try ErrorKind::Internal))?;
        let total = counts.iter().map(|(_, v)| v).sum();
        metrics.accounts_count = counts;
        metrics.accounts_count_total = total;
        Ok(())
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

    fn update_fees_balances(&self, metrics: &mut Metrics, balances: &HashMap<(BlockchainAddress, Currency), Amount>) {}

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
