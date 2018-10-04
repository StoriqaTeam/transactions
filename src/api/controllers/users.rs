use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use models::{NewUser, UpdateUser, UserId};

pub fn post_users(ctx: &Context) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<NewUser>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service.create_user(input).map_err(ectx!(catch => input_clone))
            }).and_then(|user| response_with_model(&user)),
    )
}

pub fn get_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();

    Box::new(
        users_service
            .get_user(user_id)
            .map_err(ectx!(catch))
            .and_then(|user| response_with_model(&user)),
    )
}

pub fn put_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        parse_body::<UpdateUser>(ctx.body.clone())
            .and_then(move |input| {
                let input_clone = input.clone();
                users_service.update_user(user_id, input).map_err(ectx!(catch => input_clone))
            }).and_then(|user| response_with_model(&user)),
    )
}

pub fn delete_users(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let users_service = ctx.users_service.clone();
    Box::new(
        users_service
            .delete_user(user_id)
            .map_err(ectx!(catch))
            .and_then(|user| response_with_model(&user)),
    )
}
