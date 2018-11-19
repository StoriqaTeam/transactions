use models::*;
use prelude::*;

use super::error::*;

pub trait MetricsService: Send + Sync + 'static {
    fn get_metrics(&self) -> Box<Future<Item = Metrics, Error = Error> + Send>;
}

pub struct MetricsServiceImpl;

impl MetricsService for MetricsServiceImpl {
    fn get_metrics(&self) -> Box<Future<Item = Metrics, Error = Error> + Send> {
        Box::new(Ok(Default::default()).into_future())
    }
}
