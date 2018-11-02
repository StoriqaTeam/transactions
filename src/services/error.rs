use std::fmt;
use std::fmt::Display;

use failure::{Backtrace, Context, Fail};
use validator::ValidationErrors;

use client::blockchain_gateway::ErrorKind as BlockchainClientErrorKind;
use client::keys::ErrorKind as KeysClientErrorKind;
use repos::{Error as ReposError, ErrorKind as ReposErrorKind};

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
    #[fail(display = "service error - not found")]
    NotFound,
    #[fail(display = "service error - balance failure")]
    Balance,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "service error source - r2d2")]
    R2D2,
    #[fail(display = "service error source - repos")]
    Repo,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "service error context - no auth token received")]
    NoAuthToken,
    #[fail(display = "service error context - invalid auth token")]
    InvalidToken,
    #[fail(display = "service error context - no account found")]
    NoAccount,
    #[fail(display = "service error context - no transaction found")]
    NoTransaction,
    #[fail(display = "service error context - not enough founds")]
    NotEnoughFunds,
    #[fail(display = "service error context - invalid currency")]
    InvalidCurrency,
    #[fail(display = "service error context - invalid utf8 bytes")]
    UTF8,
    #[fail(display = "service error context - failed to parse string to json")]
    Json,
    #[fail(display = "service error context - balance overflow")]
    BalanceOverflow,
    #[fail(display = "service error context - transaction between two dr accounts")]
    InvalidTransaction,
    #[fail(display = "service error context - invalid uuid")]
    InvalidUuid,
    #[fail(display = "service error context - operation not yet supproted")]
    NotSupported,
}

derive_error_impls!();

impl From<ReposError> for Error {
    fn from(e: ReposError) -> Error {
        let kind: ErrorKind = e.kind().into();
        e.context(kind).into()
    }
}

impl From<ReposErrorKind> for ErrorKind {
    fn from(e: ReposErrorKind) -> ErrorKind {
        match e {
            ReposErrorKind::Internal => ErrorKind::Internal,
            ReposErrorKind::Unauthorized => ErrorKind::Unauthorized,
            ReposErrorKind::Constraints(validation_errors) => ErrorKind::InvalidInput(validation_errors),
        }
    }
}

impl From<KeysClientErrorKind> for ErrorKind {
    fn from(err: KeysClientErrorKind) -> Self {
        match err {
            KeysClientErrorKind::Internal => ErrorKind::Internal,
            KeysClientErrorKind::Unauthorized => ErrorKind::Unauthorized,
            KeysClientErrorKind::MalformedInput => ErrorKind::MalformedInput,
        }
    }
}

impl From<BlockchainClientErrorKind> for ErrorKind {
    fn from(err: BlockchainClientErrorKind) -> Self {
        match err {
            BlockchainClientErrorKind::Internal => ErrorKind::Internal,
            BlockchainClientErrorKind::Unauthorized => ErrorKind::Unauthorized,
            BlockchainClientErrorKind::MalformedInput => ErrorKind::MalformedInput,
        }
    }
}
