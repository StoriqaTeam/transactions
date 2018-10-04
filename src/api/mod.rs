use hyper;
use hyper::{service::Service, Body, Request, Response};

use super::config::Config;
use super::utils::{log_and_capture_error, log_error, log_warn};
use client::{HttpClientImpl};
use failure::{Compat, Fail};
use futures::future;
use futures::prelude::*;
use hyper::Server;
use models::AuthError;
use std::net::SocketAddr;
use std::sync::Arc;
use utils::read_body;

mod auth;
mod controllers;
mod error;
mod requests;
mod responses;
mod utils;

use self::auth::{Authenticator, AuthenticatorImpl};
use self::controllers::*;
use self::error::*;
use services::UsersServiceImpl;

#[derive(Clone)]
pub struct ApiService {
    authenticator: Arc<dyn Authenticator>,
    server_address: SocketAddr,
    config: Config,
}

impl ApiService {
    fn from_config(config: &Config) -> Result<Self, Error> {
        let client = HttpClientImpl::new(config);
        let server_address = format!("{}:{}", config.server.host, config.server.port)
            .parse::<SocketAddr>()
            .map_err(ectx!(raw_err
                ErrorContext::Config,
                ErrorKind::Internal =>
                config.server.host,
                config.server.port
            ))?;
        let authenticator = AuthenticatorImpl::default();
        Ok(ApiService {
            config: config.clone(),
            authenticator: Arc::new(authenticator),
            server_address,
        })
    }
}

impl Service for ApiService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = Compat<Error>;
    type Future = Box<Future<Item = Response<Body>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let (parts, http_body) = req.into_parts();
        let authenticator = self.authenticator.clone();
        Box::new(
            read_body(http_body)
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                .and_then(move |body| {
                    let router = router! {
                        POST /v1/sessions => post_sessions,
                        POST /v1/sessions/oauth => post_sessions_oauth,
                        POST /v1/users => post_users,
                        POST /v1/users/confirm_email => post_users_confirm_email,
                        GET /v1/users/me => get_users_me,
                        _ => not_found,
                    };

                    let auth_result = authenticator.authenticate(&parts.headers).map_err(AuthError::new);
                    let users_service = UsersServiceImpl::new(auth_result.clone());

                    let ctx = Context {
                        body,
                        method: parts.method.clone(),
                        uri: parts.uri.clone(),
                        headers: parts.headers,
                        auth_result,
                        users_service: Arc::new(users_service),
                    };

                    debug!("Received request {}", ctx);

                    router(ctx, parts.method.into(), parts.uri.path())
                }).or_else(|e| match e.kind() {
                    ErrorKind::BadRequest => {
                        log_error(&e);
                        Ok(Response::builder()
                            .status(400)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Bad request"}"#))
                            .unwrap())
                    }
                    ErrorKind::Unauthorized => {
                        log_warn(&e);
                        Ok(Response::builder()
                            .status(401)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Unauthorized"}"#))
                            .unwrap())
                    }
                    ErrorKind::UnprocessableEntity(errors) => {
                        log_warn(&e);
                        Ok(Response::builder()
                            .status(422)
                            .header("Content-Type", "application/json")
                            .body(Body::from(format!("{}", errors)))
                            .unwrap())
                    }
                    ErrorKind::Internal => {
                        log_and_capture_error(e);
                        Ok(Response::builder()
                            .status(500)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Internal server error"}"#))
                            .unwrap())
                    }
                }),
        )
    }
}

pub fn start_server(config: Config) {
    hyper::rt::run(future::lazy(move || {
        ApiService::from_config(&config)
            .into_future()
            .and_then(move |api| {
                let api_clone = api.clone();
                let new_service = move || {
                    let res: Result<_, hyper::Error> = Ok(api_clone.clone());
                    res
                };
                let addr = api.server_address.clone();
                let server = Server::bind(&api.server_address)
                    .serve(new_service)
                    .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => addr));
                info!("Listening on http://{}", addr);
                server
            }).map_err(|e: Error| log_error(&e))
    }));
}
