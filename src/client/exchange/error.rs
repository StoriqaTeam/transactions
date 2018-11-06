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
    #[fail(display = "key client error - malformed input")]
    MalformedInput,
    #[fail(display = "key client error - unauthorized")]
    Unauthorized,
    #[fail(display = "key client error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "key client source - error inside of Hyper library")]
    Hyper,
    #[fail(display = "key client source - error parsing bytes to utf8")]
    Utf8,
    #[fail(display = "key client source - error parsing string to json")]
    Json,
}

derive_error_impls!();
