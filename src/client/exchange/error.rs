use std::fmt;
use std::fmt::Display;

use failure::{Backtrace, Context, Fail};

use client::http_client::error::ErrorKind as HttpClientErrorKind;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "exchange client error - malformed input")]
    MalformedInput,
    #[fail(display = "exchange client error - unauthorized")]
    Unauthorized,
    #[fail(display = "exchange client error - internal error")]
    Internal,
    #[fail(display = "exchange client error - bad request")]
    Validation(String),
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "exchange client source - error inside of Hyper library")]
    Hyper,
    #[fail(display = "exchange client source - error parsing bytes to utf8")]
    Utf8,
    #[fail(display = "exchange client source - error parsing string to json")]
    Json,
}

derive_error_impls!();

impl From<HttpClientErrorKind> for ErrorKind {
    fn from(err: HttpClientErrorKind) -> Self {
        match err {
            HttpClientErrorKind::Validation(s) => ErrorKind::Validation(s),
            _ => ErrorKind::Internal,
        }
    }
}
