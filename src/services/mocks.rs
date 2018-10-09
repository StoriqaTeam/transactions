use std::collections::HashMap;

use super::auth::AuthService;
use super::error::*;
use super::ServiceFuture;
use models::*;
use prelude::*;

pub struct AuthServiceMock {
    users: HashMap<AuthenticationToken, User>,
}

impl AuthServiceMock {
    pub fn new(allowed_tokens: Vec<AuthenticationToken>) -> Self {
        let mut users = HashMap::new();
        for token in allowed_tokens {
            let mut user = User::default();
            user.authentication_token = token.clone();
            users.insert(token, user);
        }
        AuthServiceMock { users }
    }
}

impl AuthService for AuthServiceMock {
    fn authenticate(&self, maybe_token: Option<AuthenticationToken>) -> ServiceFuture<User> {
        Box::new(
            maybe_token
                .and_then(|token| self.users.get(&token))
                .map(|x| x.clone())
                .ok_or(ectx!(err ErrorContext::NoAuthToken, ErrorKind::Unauthorized))
                .into_future(),
        )
    }
}
