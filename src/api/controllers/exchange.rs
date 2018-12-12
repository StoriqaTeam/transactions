use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use api::error::*;
use models::*;

pub fn post_rate(ctx: &Context) -> ControllerFuture {
    let exchange_service = ctx.exchange_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<RateInput>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        exchange_service.rate(token, input).map_err(ectx!(convert => input_clone))
                    })
                    .and_then(|rate| response_with_model(&rate))
            }),
    )
}
