use std::fmt;
use std::fmt::Display;

use diesel::result::{DatabaseErrorKind, Error as DieselError};
use failure::{Backtrace, Context, Fail};
use validator::{ValidationError, ValidationErrors};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "repo error - unauthorized")]
    Unauthorized,
    #[fail(display = "database error - constraints violation: {}", _0)]
    Constraints(ValidationErrors),
    #[fail(display = "repo error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "database source - error inside of Diesel library")]
    Diesel,
    #[fail(display = "database source - error inside of r2d2 library")]
    R2D2,
    #[fail(display = "database source - error inside postgres transaction")]
    Transaction,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "database context - error getting connection")]
    Connection,
    #[fail(display = "database context - balance overflow")]
    BalanceOverflow,
}

derive_error_impls!();

impl<'a> From<&'a DieselError> for ErrorKind {
    fn from(e: &DieselError) -> Self {
        match e {
            DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, ref info) => {
                let mut errors = ValidationErrors::new();
                let mut error = ValidationError::new("not unique");
                let message: &str = info.message();
                let details: &str = info.details().unwrap_or("no details");
                error.add_param("message".into(), &message);
                error.add_param("details".into(), &details);
                errors.add("database", error);
                ErrorKind::Constraints(errors)
            }
            _ => ErrorKind::Internal,
        }
    }
}
