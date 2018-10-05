use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "jwt auth error - unauthorized")]
    Unauthorized,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "jwt auth error - no auth header supplied")]
    NoAuthHeader,
    #[fail(display = "jwt auth error - invalid auth header, not starting with Bearer")]
    InvalidBearer,
    #[fail(display = "jwt auth error - couldn't parse auth header to string")]
    ParseAuthHeader,
    #[fail(display = "jwt auth error - no bearer field supplied")]
    NoBearerField,
    #[fail(display = "jwt auth error - error inside json web token crate")]
    JsonWebToken,
}

derive_error_impls!();
