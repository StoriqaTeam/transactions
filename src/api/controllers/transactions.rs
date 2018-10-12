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

pub fn post_transactions_local(ctx: &Context) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PostTransactionsLocalRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        transactions_service
                            .create_transaction_local(token, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|transaction| response_with_model(&TransactionsResponse::from(transaction)))
            }),
    )
}

pub fn post_transactions_deposit(ctx: &Context) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PostTransactionsDepositRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        transactions_service
                            .deposit_founds(token, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|transaction| response_with_model(&TransactionsResponse::from(transaction)))
            }),
    )
}

pub fn post_transactions_withdraw(ctx: &Context) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PostTransactionsWithdrawRequest>(body)
                    .and_then(move |input| {
                        let input_clone = input.clone();
                        transactions_service
                            .withdraw(token, input.into())
                            .map_err(ectx!(convert => input_clone))
                    }).and_then(|transaction| response_with_model(&TransactionsResponse::from(transaction)))
            }),
    )
}

pub fn get_users_transactions(ctx: &Context, user_id: UserId) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    let path_and_query = ctx.uri.path_and_query();
    let path_and_query_clone = ctx.uri.path_and_query();
    Box::new(
        ctx.uri
            .query()
            .ok_or(ectx!(err ErrorContext::RequestMissingQuery, ErrorKind::BadRequest => path_and_query))
            .and_then(|query| {
                serde_qs::from_str::<GetUsersTransactionsParams>(query).map_err(|e| {
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
                        transactions_service
                            .get_transactions_for_user(token, user_id, input.offset, input.limit)
                            .map_err(ectx!(convert => input_clone))
                    })
            }).and_then(|transactions| {
                let transactions: Vec<TransactionsResponse> = transactions.into_iter().map(From::from).collect();
                response_with_model(&transactions)
            }),
    )
}

pub fn get_transactions(ctx: &Context, transaction_id: TransactionId) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                transactions_service
                    .get_transaction(token, transaction_id)
                    .map_err(ectx!(convert))
                    .and_then(|transaction| response_with_model(&transaction.map(|transaction| TransactionsResponse::from(transaction))))
            }),
    )
}

pub fn get_accounts_transactions(ctx: &Context, account_id: AccountId) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                transactions_service
                    .get_account_transactions(token, account_id)
                    .map_err(ectx!(convert))
            }).and_then(|transactions| {
                let transactions: Vec<TransactionsResponse> = transactions.into_iter().map(From::from).collect();
                response_with_model(&transactions)
            }),
    )
}

pub fn put_transactions_status(ctx: &Context, transaction_id: TransactionId) -> ControllerFuture {
    let transactions_service = ctx.transactions_service.clone();
    let maybe_token = ctx.get_auth_token();
    let body = ctx.body.clone();
    Box::new(
        maybe_token
            .ok_or_else(|| ectx!(err ErrorContext::Token, ErrorKind::Unauthorized))
            .into_future()
            .and_then(move |token| {
                parse_body::<PutTransactionsRequest>(body)
                    .and_then(move |input| {
                        let new_status = input.into();
                        transactions_service
                            .update_transaction_status(token, transaction_id, new_status)
                            .map_err(ectx!(convert => transaction_id, new_status))
                    }).and_then(|transaction| response_with_model(&TransactionsResponse::from(transaction)))
            }),
    )
}
