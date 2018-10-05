use failure::{Backtrace, Context, Fail};
use services::ErrorKind as ServiceErrorKind;
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
    #[fail(display = "controller error - unauthorized")]
    Unauthorized,
    #[fail(display = "controller error - bad request")]
    BadRequest,
    #[fail(display = "controller error - unprocessable entity")]
    UnprocessableEntity(ValidationErrors),
    #[fail(display = "controller error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "controller source - error inside of Hyper library")]
    Hyper,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "controller source - error parsing config data")]
    Config,
    #[fail(display = "controller source - error converting json data from request")]
    RequestJson,
    #[fail(display = "controller source - error parsing bytes into utf8 from request")]
    RequestUTF8,
    #[fail(display = "controller source - error converting json data from request")]
    ResponseJson,
}

derive_error_impls!();

impl From<ServiceErrorKind> for ErrorKind {
    fn from(err: ServiceErrorKind) -> Self {
        match err {
            ServiceErrorKind::Internal => ErrorKind::Internal,
            ServiceErrorKind::Unauthorized => ErrorKind::Unauthorized,
            ServiceErrorKind::MalformedInput => ErrorKind::BadRequest,
            ServiceErrorKind::InvalidInput(validation_errors) => ErrorKind::UnprocessableEntity(validation_errors),
        }
    }
}
