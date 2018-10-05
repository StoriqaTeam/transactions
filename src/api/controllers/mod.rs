use std::fmt::{self, Display};
use std::sync::Arc;

use futures::prelude::*;
use hyper::{header::HeaderValue, Body, HeaderMap, Method, Response, Uri};

use super::error::*;
use models::AuthResult;
use services::UsersService;

mod fallback;
mod users;

pub use self::fallback::*;
pub use self::users::*;

pub type ControllerFuture = Box<Future<Item = Response<Body>, Error = Error> + Send>;

#[derive(Clone)]
pub struct Context {
    pub body: Vec<u8>,
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap<HeaderValue>,
    pub auth_result: AuthResult,
    pub users_service: Arc<dyn UsersService>,
}

impl Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "{} {}, headers: {:#?}, auth_result:{:?}, body: {:?}",
            self.method,
            self.uri,
            self.headers,
            self.auth_result,
            String::from_utf8(self.body.clone()).ok()
        ))
    }
}
