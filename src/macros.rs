//! Module containing all macros
//!

/// `ectx!` is a macro for converting errors from submodule to a module level.
///
/// The most basic way you handle error is
/// ```ignore
/// let res = dangerous_operation(param1)
///     .map_err(|e|
///          e.context(format!("Called at line {}, filename: {}, with args: param1: {}", line!(), file!(), param1))
///              .context(ErrorContext::Some)
///              .context(ErrorKind::Internal)
///              .into()
///     )
/// ```
///
/// ectx reduce all this boilerplate. Instead you just do:
/// ```ignore
/// let res = dangerous_operation(param1)
///     .map_err(ectx!(ErrorContext::Some, ErrorKind::Internal => param1)
/// ```
///
/// ## Syntax:
///
/// `ectx(try? convert? err? context1, context2, ... => arg1, arg2, ...)`
/// where `?` means zero or one times
///
/// 1. `try`
/// Use this keyword if you want to avoid final `.into()` conversion.
/// This is useful if you're using this macro with try (question mark) syntax.
/// Example:
/// ```ignore
/// let server_address = format!("{}:{}", config.server.host, config.server.port)
///     .parse::<SocketAddr>()
///     .map_err(ectx!(try
///          ErrorContext::Config,
///          ErrorKind::Internal =>
///          config.server.host,
///          config.server.port
///     ))?;
/// ```
///
/// If you omit `try` param here then compiler will fail to resolve type, since
/// `ectx!` is using `into` by default and `try` is using `into`.
///
/// 2. `convert`
/// Use this keyword if you want to convert source ErrorKind to your module level ErrorKind.
/// To use this keyword you need to implement `From` conversion for these kinds
///
/// 3. `err`
/// By default `ectx!` returns closure, that is used in map_err, since it's the most common use case.
/// If you want to get raw error, use `err key word`. E.g. these two calls are equivalent:
///
/// ```ignore
/// map_err(ectx!(ErrorKind::SomeErr))
/// ```
///
/// and
///
/// ```ignore
/// map_err(|e| ectx!(err ErrorKind::SomeErr))
/// ```
///
#[macro_export]
macro_rules! ectx {
    (try err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let mut msg = "at ".to_string();
        msg.push_str(&format!("{}:{}", file!(), line!()));
        $(
            $(
                let arg = format!("\nwith args - {}: {:#?}", stringify!($arg), $arg);
                msg.push_str(&arg);
            )*
        )*
        $e.context(msg)$(.context($context))*
    }};

    (err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let err = ectx!(try err $e $(,$context)* $(=> $($arg),*)*);
        err.into()
    }};

    (try convert err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let e = $e.kind().into();
        ectx!(try err $e $(,$context)*, e $(=> $($arg),*)*)
    }};

    (convert err $e:expr $(,$context:expr)* $(=> $($arg:expr),*)*) => {{
        let e = $e.kind().into();
        ectx!(err $e $(,$context)*, e $(=> $($arg),*)*)
    }};

    (try convert $($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(try convert err e $(,$context)* $(=> $($arg),*)*)
        }
    }};

    (convert $($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(convert err e $(,$context)* $(=> $($arg),*)*)
        }
    }};

    (try $($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(try err e $(,$context)* $(=> $($arg),*)*)
        }
    }};

    ($($context:expr),* $(=> $($arg:expr),*)*) => {{
        move |e| {
            ectx!(err e $(,$context)* $(=> $($arg),*)*)
        }
    }};
}

/// Derives `FromSql` and `ToSql` for a newtype.
///
/// This macro simply forwards `FromSql` and `ToSql` functions to underlying type.
///
/// ## Usage:
/// derive_newtype_sql!(enclosing_module_name, sql_type, derived_type, derived_type);
///
/// 1. `enclosing_module_name` - this just a random name that must be unique in current file
/// As a convention use snake_case name of a derived type here
///
/// 2. sql_type - this is a sql type from diesel::sql_types that is mapped to our derived type.
/// Before using this macro, make sure you imported it in scope.
///
/// 3. derived_type - the name of the type we're deriving
/// 4. derived_type - same as 3. (needed for correct working of macro)
///
/// ## Examples:
///
/// ```ignore
/// use diesel::sql_types::{Uuid as SqlUuid, VarChar};
/// use uuid::Uuid;
/// #[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone)]
/// #[sql_type = "SqlUuid"]
/// pub struct UserId(Uuid);
/// derive_newtype_sql!(user_id, SqlUuid, UserId, UserId);
/// ```
///
/// ```ignore
/// use diesel::sql_types::{Uuid as SqlUuid, VarChar};
/// #[derive(Deserialize, FromSqlRow, AsExpression, Clone)]
/// #[sql_type = "VarChar"]
/// pub struct AuthenticationToken(String);
/// derive_newtype_sql!(authentication_token, VarChar, AuthenticationToken, AuthenticationToken);
/// ```
///
#[macro_export]
macro_rules! derive_newtype_sql {
    ($mod_name:ident, $sql_type:ty, $type:ty, $constructor:expr) => {
        mod $mod_name {
            use super::*;
            use diesel::deserialize::{self, FromSql};
            use diesel::pg::Pg;
            use diesel::serialize::{self, Output, ToSql};
            use std::io::Write;

            impl FromSql<$sql_type, Pg> for $type {
                fn from_sql(data: Option<&[u8]>) -> deserialize::Result<Self> {
                    FromSql::<$sql_type, Pg>::from_sql(data).map($constructor)
                }
            }

            impl ToSql<$sql_type, Pg> for $type {
                fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> serialize::Result {
                    ToSql::<$sql_type, Pg>::to_sql(&self.0, out)
                }
            }
        }
    };
}

/// Masks logs for this type by deriving `Debug` and `Display` appropriately.
///
/// This macro displays type as `*******`. Use it for sensitive info like passwords, tokens, etc...
#[macro_export]
macro_rules! mask_logs {
    ($type:ty) => {
        impl Debug for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                f.write_str("********")
            }
        }

        impl Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                f.write_str("********")
            }
        }
    };
}

/// Derives different `Error` trait implementations.
///
/// Implemetations according to this `Failure` crate [approach](https://boats.gitlab.io/failure/error-errorkind.html)
///
/// ## Examples:
///
/// ```ignore
/// use failure::{Backtrace, Context, Fail};
/// use services::ErrorKind as ServiceErrorKind;
/// use std::fmt;
/// use std::fmt::Display;
/// use validator::ValidationErrors;
///
/// #[derive(Debug)]
/// pub struct Error {
///     inner: Context<ErrorKind>,
/// }
///
/// #[allow(dead_code)]
/// #[derive(Clone, Debug, Fail)]
/// pub enum ErrorKind {
///     #[fail(display = "controller error - unauthorized")]
///     Unauthorized,
///     #[fail(display = "controller error - bad request")]
///     BadRequest,
///     #[fail(display = "controller error - unprocessable entity")]
///     UnprocessableEntity(ValidationErrors),
///     #[fail(display = "controller error - internal error")]
///     Internal,
/// }
///
/// #[allow(dead_code)]
/// #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
/// pub enum ErrorSource {
///     #[fail(display = "controller source - error inside of Hyper library")]
///     Hyper,
/// }
///
/// #[allow(dead_code)]
/// #[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
/// pub enum ErrorContext {
///     #[fail(display = "controller source - error parsing config data")]
///     Config,
///     #[fail(display = "controller source - error converting json data from request")]
///     RequestJson,
///     #[fail(display = "controller source - error parsing bytes into utf8 from request")]
///     RequestUTF8,
///     #[fail(display = "controller source - error converting json data from request")]
///     ResponseJson,
/// }
///
/// derive_error_impls!();
/// ```
///
#[macro_export]
macro_rules! derive_error_impls {
    () => {
        #[allow(dead_code)]
        impl Error {
            pub fn kind(&self) -> ErrorKind {
                self.inner.get_context().clone()
            }
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

        impl From<ErrorKind> for Error {
            fn from(kind: ErrorKind) -> Error {
                Error {
                    inner: Context::new(kind),
                }
            }
        }

        impl From<Context<ErrorKind>> for Error {
            fn from(inner: Context<ErrorKind>) -> Error {
                Error { inner: inner }
            }
        }
    };
}
