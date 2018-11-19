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
    #[fail(display = "exchange client error - malformed input")]
    MalformedInput,
    #[fail(display = "exchange client error - unauthorized")]
    Unauthorized,
    #[fail(display = "exchange client error - internal error")]
    Internal,
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
