use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use api::requests::*;
use api::responses::*;
use models::*;

pub fn post_users(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PostUsersRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service.create_user(input.into()).map_err(ectx!(convert => input_clone))
            }).and_then(|user| response_with_model(&UsersResponse::from(user))),
    )
}

pub fn get_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();

    Box::new(
        users_service
            .get_user(user_id)
            .map_err(ectx!(convert))
            .and_then(|user| response_with_model(&user.map(|user| UsersResponse::from(user)))),
    )
}

pub fn put_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<PutUsersRequest>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service
                    .update_user(user_id, input.into())
                    .map_err(ectx!(convert => input_clone))
            }).and_then(|user| response_with_model(&UsersResponse::from(user))),
    )
}

pub fn delete_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        users_service
            .delete_user(user_id)
            .map_err(ectx!(convert))
            .and_then(|user| response_with_model(&UsersResponse::from(user))),
    )
}
