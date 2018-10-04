
use super::error::*;
use models::*;
use prelude::*;

pub trait UsersService: Send + Sync + 'static {
    fn me(&self) -> Box<Future<Item = User, Error = Error> + Send>;
}

pub struct UsersServiceImpl {
    auth_result: AuthResult,
}

impl UsersServiceImpl {
    pub fn new(auth_result: AuthResult) -> Self {
        UsersServiceImpl {
            auth_result,
        }
    }
}

impl UsersService for UsersServiceImpl {
    fn me(&self) -> Box<Future<Item = User, Error = Error> + Send> {
        unimplemented!()
    }
}
