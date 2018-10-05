use client::ErrorKind as ClientErrorKind;
use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::fmt::Display;
use validator::ValidationErrors;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "service error - unauthorized")]
    Unauthorized,
    #[fail(display = "service error - malformed input")]
    MalformedInput,
    #[fail(display = "service error - invalid input, errors: {}", _0)]
    InvalidInput(ValidationErrors),
    #[fail(display = "service error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "service error context - internal error")]
    Internal,
}

derive_error_impls!();

impl From<ClientErrorKind> for ErrorKind {
    fn from(err: ClientErrorKind) -> Self {
        match err {
            ClientErrorKind::Internal => ErrorKind::Internal,
            ClientErrorKind::Unauthorized => ErrorKind::Unauthorized,
            _ => ErrorKind::Internal,
        }
    }
}
