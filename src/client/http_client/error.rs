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
    #[fail(display = "http client error - bad request")]
    BadRequest,
    #[fail(display = "http client error - unauthorized")]
    Unauthorized,
    #[fail(display = "http client error - not found")]
    NotFound,
    #[fail(display = "http client error - unprocessable entity")]
    UnprocessableEntity,
    #[fail(display = "http client error - internal server error")]
    InternalServer,
    #[fail(display = "http client error - bad gateway")]
    BadGateway,
    #[fail(display = "http client error - timeout")]
    GatewayTimeout,
    #[fail(display = "http client error - unknown server error status")]
    UnknownServerError,
    #[fail(display = "http client error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "http client source - error inside of Hyper library")]
    Hyper,
    #[fail(display = "http client source - server returned response with error")]
    Server,
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
