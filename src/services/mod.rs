mod accounts;
mod auth;
mod error;
#[cfg(test)]
mod mocks;
mod rabbit;
mod transactions;
mod users;

pub use self::accounts::*;
pub use self::auth::*;
pub use self::error::*;
#[cfg(test)]
pub use self::mocks::*;
pub use self::rabbit::*;
pub use self::transactions::*;
pub use self::users::*;

use prelude::*;

type ServiceFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
