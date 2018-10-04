mod error;
mod responses;

pub use self::error::*;
use self::responses::*;
use super::HttpClient;
use config::Config;
use failure::Fail;
use futures::prelude::*;
use hyper::Method;
use hyper::{Body, Request};
use models::StoriqaJWT;
use models::*;
use serde::Deserialize;
use serde_json;
use std::sync::Arc;
use utils::read_body;

pub trait StoriqaClient: Send + Sync + 'static {
    fn get_jwt(&self, email: String, password: Password) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send>;
    fn get_jwt_by_oauth(&self, oauth_token: OauthToken, oauth_provider: Provider) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send>;
    fn create_user(&self, new_user: NewUser) -> Box<Future<Item = User, Error = Error> + Send>;
    fn confirm_email(&self, token: EmailConfirmToken) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send>;
    fn me(&self, token: StoriqaJWT) -> Box<Future<Item = User, Error = Error> + Send>;
}

pub struct StoriqaClientImpl {
    cli: Arc<HttpClient>,
    storiqa_url: String,
}

impl StoriqaClientImpl {
    pub fn new<C: HttpClient>(config: &Config, cli: C) -> Self {
        Self {
            cli: Arc::new(cli),
            storiqa_url: config.client.storiqa_url.clone(),
        }
    }

    fn exec_query<T: for<'de> Deserialize<'de> + Send>(
        &self,
        query: &str,
        token: Option<StoriqaJWT>,
    ) -> impl Future<Item = T, Error = Error> + Send {
        let query = query.to_string();
        let query1 = query.clone();
        let query2 = query.clone();
        let query3 = query.clone();
        let cli = self.cli.clone();
        let query = query.replace("\n", "");
        let body = format!(
            r#"
                {{
                    "operationName": "M",
                    "query": "{}",
                    "variables": null
                }}
            "#,
            query
        );
        let mut builder = Request::builder();
        builder.uri(self.storiqa_url.clone()).method(Method::POST);
        if let Some(token) = token {
            builder.header("Authorization", format!("Bearer {}", token.inner()));
        }
        builder
            .body(Body::from(body))
            .map_err(ectx!(ErrorSource::Hyper, ErrorKind::MalformedInput => query3))
            .into_future()
            .and_then(move |req| cli.request(req).map_err(ectx!(ErrorKind::Internal => query1)))
            .and_then(move |resp| read_body(resp.into_body()).map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => query2)))
            .and_then(|bytes| {
                let bytes_clone = bytes.clone();
                String::from_utf8(bytes).map_err(ectx!(ErrorSource::Utf8, ErrorKind::Internal => bytes_clone))
            }).and_then(|string| serde_json::from_str::<T>(&string).map_err(ectx!(ErrorSource::Json, ErrorKind::Internal => string)))
    }
}

impl StoriqaClient for StoriqaClientImpl {
    fn get_jwt(&self, email: String, password: Password) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send> {
        let query = format!(
            r#"
                mutation M {{
                    getJWTByEmail(input: {{email: \"{}\", password: \"{}\", clientMutationId:\"\"}}) {{
                        token
                    }}
                }}
            "#,
            email,
            password.inner()
        );
        Box::new(
            self.exec_query::<GetJWTResponse>(&query, None)
                .and_then(|resp| {
                    resp.data
                        .clone()
                        .ok_or(ectx!(err ErrorContext::NoGraphQLData, ErrorKind::Unauthorized => resp))
                }).map(|resp_data| resp_data.get_jwt_by_email.token),
        )
    }

    fn get_jwt_by_oauth(&self, oauth_token: OauthToken, oauth_provider: Provider) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send> {
        let query = format!(
            r#"
                mutation M {{
                    getJWTByProvider(input: {{token: \"{}\", provider: {}, clientMutationId:\"\"}}) {{
                        token
                    }}
                }}
            "#,
            oauth_token,
            format!("{}", oauth_provider).to_uppercase(),
        );
        info!("{}", query);
        Box::new(
            self.exec_query::<GetJWTByProviderResponse>(&query, None)
                .and_then(|resp| {
                    resp.data
                        .clone()
                        .ok_or(ectx!(err ErrorContext::NoGraphQLData, ErrorKind::Unauthorized => resp))
                }).map(|resp_data| resp_data.get_jwt_by_provider.token),
        )
    }

    fn create_user(&self, new_user: NewUser) -> Box<Future<Item = User, Error = Error> + Send> {
        let NewUser {
            email,
            password,
            first_name,
            last_name,
        } = new_user;
        let query = format!(
            r#"
                mutation M {{
                    createUser(input: {{email: \"{}\", password: \"{}\", firstName: \"{}\", lastName: \"{}\", clientMutationId:\"\"}}) {{
                        email
                        firstName
                        lastName
                    }}
                }}
            "#,
            email,
            password.inner(),
            first_name,
            last_name,
        );
        Box::new(
            self.exec_query::<CreateUserResponse>(&query, None)
                .and_then(|resp| {
                    resp.data
                        .clone()
                        .ok_or(ectx!(err ErrorContext::NoGraphQLData, ErrorKind::Unauthorized => resp))
                }).map(|resp_data| resp_data.create_user),
        )
    }

    fn me(&self, token: StoriqaJWT) -> Box<Future<Item = User, Error = Error> + Send> {
        let query = r#"
                query M {
                    me {
                        email
                        firstName
                        lastName
                        phone
                    }
                }
            "#;
        Box::new(
            self.exec_query::<MeResponse>(&query, Some(token))
                .and_then(|resp| {
                    resp.data
                        .clone()
                        .ok_or(ectx!(err ErrorContext::NoGraphQLData, ErrorKind::Unauthorized => resp))
                }).map(|resp_data| resp_data.me),
        )
    }

    fn confirm_email(&self, token: EmailConfirmToken) -> Box<Future<Item = StoriqaJWT, Error = Error> + Send> {
        let query = format!(
            r#"
                mutation M {{
                    verifyEmail(input: {{token: \"{}\", clientMutationId:\"\"}}) {{
                        token
                    }}
                }}
            "#,
            token,
        );
        Box::new(
            self.exec_query::<GetJWTResponse>(&query, None)
                .and_then(|resp| {
                    resp.data
                        .clone()
                        .ok_or(ectx!(err ErrorContext::NoGraphQLData, ErrorKind::Unauthorized => resp))
                }).map(|resp_data| resp_data.get_jwt_by_email.token),
        )
    }
}
