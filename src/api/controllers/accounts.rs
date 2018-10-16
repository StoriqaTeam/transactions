use failure::Fail;
use futures::prelude::*;

use super::super::utils::{parse_body, response_with_model};
use super::Context;
use super::ControllerFuture;
use api::error::*;
use api::requests::*;
use api::responses::*;
use models::*;
use serde_qs;

pub fn post_accounts(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PostAccountsRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        accounts_service
                            .create_account(token, user_id, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}

pub fn get_users_accounts(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    let path_and_query = ctx.uri.path_and_query();
    let path_and_query_clone = ctx.uri.path_and_query();
    Box::new(
        ctx.uri
            .query()
            .ok_or(ectx!(err ErrorContext::RequestMissingQuery, ErrorKind::BadRequest => path_and_query))
            .and_then(|query| {
                serde_qs::from_str::<GetUsersAccountsParams>(query).map_err(|e| {
                    let e = format_err!("{}", e);
                    ectx!(err e, ErrorContext::RequestQueryParams, ErrorKind::BadRequest => path_and_query_clone)
                })
            }).into_future()
            .and_then(move |input| {
                maybe_token
                    .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
                    .into_future()
                    .and_then(move |token| {
                        let input_clone = input.clone();
                        accounts_service
                            .get_accounts_for_user(token, user_id, input.offset, input.limit)
                            .map_err(ectx!(convert => input_clone))
                    })
            }).and_then(|accounts| {
                let accounts: Vec<AccountsResponse> = accounts.into_iter().map(From::from).collect();
                response_with_model(&accounts)
            }),
    )
}

pub fn get_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                accounts_service
                    .get_account(token, account_id)
                    .map_err(ectx!(convert))
                    .and_then(|account| response_with_model(&account.map(AccountsResponse::from)))
            }),
    )
}

pub fn put_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PutAccountsRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        accounts_service
                            .update_account(token, account_id, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}

pub fn delete_accounts(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                accounts_service
                    .delete_account(token, account_id)
                    .map_err(ectx!(convert))
                    .and_then(|account| response_with_model(&AccountsResponse::from(account)))
            }),
    )
}

pub fn get_users_balances(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                accounts_service
                    .get_user_balance(token, user_id)
                    .map_err(ectx!(convert))
                    .and_then(|balance| response_with_model(&BalancesResponse::from(balance)))
            }),
    )
}
pub fn get_accounts_balances(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let accounts_service = ctx.accounts_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                accounts_service
                    .get_account_balance(token, account_id)
                    .map_err(ectx!(convert))
                    .and_then(|balance| response_with_model(&BalanceResponse::from(balance)))
            }),
    )
}
