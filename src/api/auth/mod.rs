mod error;

use self::error::*;
use failure::Fail;
use hyper::header::{HeaderMap, AUTHORIZATION};
use models::*;

pub trait Authenticator: Send + Sync + 'static {
    fn authenticate(&self, headers: &HeaderMap) -> Result<Auth, Error>;
}

#[derive(Default)]
pub struct AuthenticatorImpl;

impl Authenticator for AuthenticatorImpl {
    fn authenticate(&self, headers: &HeaderMap) -> Result<Auth, Error> {
        let headers_clone = headers.clone();
        let header = headers
            .get(AUTHORIZATION)
            .ok_or(ectx!(err_contexts ErrorContext::NoAuthHeader, ErrorKind::Unauthorized => headers_clone))?
            .to_str()
            .map_err(ectx!(raw_err ErrorContext::ParseAuthHeader, ErrorKind::Unauthorized))?;

        let len = "Bearer ".len();
        if (header.len() > len) && header.starts_with("Bearer ") {
            Ok(Auth {
                token: StoriqaJWT::new(header[len..].to_string()),
            })
        } else {
            Err(ectx!(err ErrorContext::InvalidBearer, ErrorKind::Unauthorized => header))
        }
    }
}
