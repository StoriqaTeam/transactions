use super::super::requests::*;
use super::super::responses::*;
use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use failure::Fail;
use futures::prelude::*;

pub fn post_sessions(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostSessionsRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service
                    .get_jwt(input.email, input.password)
                    .map_err(ectx!(catch => input_clone))
            }).and_then(|jwt| {
                let model = PostSessionsResponse { token: jwt };
                response_with_model(&model)
            }),
    )
}

pub fn post_sessions_oauth(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostSessionsOauthRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service
                    .get_jwt_by_oauth(input.oauth_token, input.oauth_provider)
                    .map_err(ectx!(catch => input_clone))
            }).and_then(|jwt| {
                let model = PostSessionsResponse { token: jwt };
                response_with_model(&model)
            }),
    )
}

pub fn post_users(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostUsersRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service
                    .create_user(input.email, input.password, input.first_name, input.last_name)
                    .map_err(ectx!(catch => input_clone))
            }).and_then(|user| response_with_model(&user)),
    )
}

pub fn post_users_confirm_email(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostUsersConfirmEmailRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service
                    .confirm_email(input.email_confirm_token)
                    .map_err(ectx!(catch => input_clone))
            }).and_then(|token| response_with_model(&token)),
    )
}

pub fn get_users_me(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(users_service.me().map_err(ectx!(catch)).and_then(|user| response_with_model(&user)))
}
