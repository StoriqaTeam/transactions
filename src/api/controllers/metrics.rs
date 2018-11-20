use super::super::utils::response_with_model;
use super::Context;
use super::ControllerFuture;
use prelude::*;

pub fn get_metrics(ctx: &Context) -> ControllerFuture {
    let metrics_service = ctx.metrics_service.clone();
    Box::new(
        metrics_service
            .get_metrics()
            .map_err(ectx!(convert))
            .and_then(|metrics| response_with_model(&metrics)),
    )
}
