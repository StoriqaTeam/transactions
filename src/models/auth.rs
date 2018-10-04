use super::storiqa_jwt::StoriqaJWT;
use prelude::*;
use std::{fmt, fmt::Display};
use utils::format_error;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    pub token: StoriqaJWT,
}

pub type AuthResult = Result<Auth, AuthError>;

#[derive(Debug, Clone)]
pub struct AuthError(String);

impl AuthError {
    pub fn new<F: Fail>(fail: F) -> Self {
        AuthError(format_error(&fail))
    }
}

impl Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Fail for AuthError {}
