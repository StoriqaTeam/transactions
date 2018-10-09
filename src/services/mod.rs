mod auth;
mod error;
#[cfg(test)]
mod mocks;
mod users;
mod accounts;

pub use self::auth::*;
pub use self::error::*;
#[cfg(test)]
pub use self::mocks::*;
pub use self::users::*;
pub use self::accounts::*;

use prelude::*;

type ServiceFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
