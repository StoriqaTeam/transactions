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
    #[fail(display = "storiqa client error - malformed input")]
    MalformedInput,
    #[fail(display = "storiqa client error - unauthorized")]
    Unauthorized,
    #[fail(display = "storiqa client error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "storiqa client source - error inside of Hyper library")]
    Hyper,
    #[fail(display = "storiqa client source - error parsing bytes to utf8")]
    Utf8,
    #[fail(display = "storiqa client source - error parsing string to json")]
    Json,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "storiqa client source - no data returned from graphql")]
    NoGraphQLData,
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

#[allow(dead_code)]
impl Error {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error { inner: Context::new(kind) }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner: inner }
    }
}
