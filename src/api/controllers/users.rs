use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use api::error::*;
use api::requests::*;
use api::responses::*;

pub fn post_users(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostUsersRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service.create_user(input.into()).map_err(ectx!(convert => input_clone))
            })
            .and_then(|user| response_with_model(&UsersResponse::from(user))),
    )
}

pub fn get_users_me(ctx: &Context) -> ControllerFuture {
    let maybe_token = ctx.get_auth_token();
    let users_service = ctx.users_service.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |maybe_token| users_service.find_user_by_authentication_token(maybe_token).map_err(ectx!(convert)))
            .and_then(|user| response_with_model(&user.map(UsersResponse::from))),
    )
}
