use super::Context;
use super::ControllerFuture;
use futures::prelude::*;
use hyper::{Body, Response};

pub fn not_found(ctx: &Context) -> ControllerFuture {
    warn!("Requested url `{}` not found", ctx.uri);
    Box::new(
        Ok(Response::builder()
            .status(404)
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"description": "Not found"}"#))
            .unwrap()).into_future(),
    )
}
