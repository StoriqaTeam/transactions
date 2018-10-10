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
    pub fn new(allowed_tokens: Vec<(AuthenticationToken, UserId)>) -> Self {
        let mut users = HashMap::new();
        for (token, id) in allowed_tokens {
            let mut user = User::default();
            user.authentication_token = token.clone();
            user.id = id;
            users.insert(token, user);
        }
        AuthServiceMock { users }
    }
}

impl AuthService for AuthServiceMock {
    fn authenticate(&self, token: AuthenticationToken) -> ServiceFuture<User> {
        Box::new(
            self.users
                .get(&token)
                .map(|x| x.clone())
                .ok_or(ectx!(err ErrorContext::InvalidToken, ErrorKind::Unauthorized))
                .into_future(),
        )
    }
}
