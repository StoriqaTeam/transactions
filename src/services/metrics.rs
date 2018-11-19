use std::sync::Arc;

use client::BlockchainClient;
use config::Config;
use models::*;
use prelude::*;
use repos::{AccountsRepo, DbExecutor, TransactionsRepo};

use super::error::*;

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
        Box::new(self.db_executor.execute_transaction(move || {
            let mut metrics: Metrics = Default::default();
            self_clone.update_counts(&mut metrics)?;
            Ok(metrics)
        }))
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
}
