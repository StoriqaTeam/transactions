pub use diesel::prelude::*;
pub use failure::Fail;
pub use futures::prelude::*;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use r2d2::Pool;

pub type PgConnectionPool = Pool<ConnectionManager<PgConnection>>;
