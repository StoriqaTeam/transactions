// use failure::Fail;
// use futures::prelude::*;

// use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
// use api::error::*;
// use api::requests::*;
// use api::responses::*;

pub fn post_fees(_ctx: &Context) -> ControllerFuture {
    unimplemented!()
    // get fees is not defined

    // let transactions_service = ctx.transactions_service.clone();
    // let maybe_token = ctx.get_auth_token();
    // let body = ctx.body.clone();
    // Box::new(
    //     maybe_token
    //         .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
    //         .into_future()
    //         .and_then(move |_token| {
    //             parse_body::<PostFeesRequest>(body).and_then(move |fees| {
    //                 let fees_clone = fees.clone();
    //                 transactions_service
    //                     .get_fees(fees.into())
    //                     .map_err(ectx!(convert => fees_clone))
    //                     .and_then(|fees| response_with_model(&FeesResponse::from(fees)))
    //             })
    //         }),
    // )
}
