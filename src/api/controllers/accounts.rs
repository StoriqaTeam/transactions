use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use api::error::*;
use api::requests::*;
use api::responses::*;
use models::*;

pub fn post_users_accounts(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |maybe_token| {
                parse_body::<PostAccountsRequest>(ctx.body.clone())
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        accounts_service
                            .create_account(maybe_token, input)
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}

pub fn get_users_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();

    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |maybe_token| {
                accounts_service
                    .get_account(maybe_token, account_id)
                    .map_err(ectx!(convert))
                    .and_then(|account| response_with_model(&account.map(|account| AccountsResponse::from(account))))
            }),
    )
}

pub fn put_users_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |maybe_token| {
                parse_body::<PutAccountsRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        accounts_service
                            .update_account(maybe_token, account_id, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}

pub fn delete_users_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |maybe_token| {
                accounts_service
                    .delete_account(maybe_token, account_id)
                    .map_err(ectx!(convert))
                    .and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}
