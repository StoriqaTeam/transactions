use std::fmt;
use std::fmt::Display;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

use failure::{Backtrace, Context, Fail};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "rabbit error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "rabbit error source - io error")]
    Io,
    #[fail(display = "rabbit error source - timeout error")]
    Timeout,
    #[fail(display = "rabbit error source - lapin lib")]
    Lapin,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "rabbit error context - error establishing TCP/IP connection")]
    TcpConnection,
    #[fail(display = "rabbit error context - error parsing RabbitMQ url")]
    RabbitUrl,
    #[fail(display = "rabbit error context - error establishing RabbitMQ connection")]
    RabbitConnection,
    #[fail(display = "rabbit error context - error creating RabbitMQ channel")]
    RabbitChannel,
    #[fail(display = "rabbit error context - error acquiring heartbeat handle")]
    HeartbeatHandle,
    #[fail(display = "rabbit error context - error during heartbeat")]
    Heartbeat,
    #[fail(display = "rabbit error context - connection timeout")]
    ConnectionTimeout,
    #[fail(display = "rabbit error context - attempted to connect again in process of establishing a connection")]
    AlreadyConnecting,
    #[fail(display = "rabbit error context - attempted to close the channel, but failed")]
    ChannelClose,
}

derive_error_impls!();

impl From<Error> for IoError {
    fn from(_: Error) -> IoError {
        IoErrorKind::Other.into()
    }
}
