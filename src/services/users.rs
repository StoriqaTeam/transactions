use futures::IntoFuture;
use validator::Validate;

use super::error::*;
use super::*;
use models::*;
use prelude::*;

pub trait UsersService: Send + Sync + 'static {
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send>;
    fn get_user(&self, user_id: UserId) -> Box<Future<Item = Option<User>, Error = Error> + Send>;
    fn update_user(&self, user_id: UserId, payload: UpdateUser) -> Box<Future<Item = User, Error = Error> + Send>;
    fn delete_user(&self, user_id: UserId) -> Box<Future<Item = User, Error = Error> + Send>;
}

impl UsersService for Service {
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send> {
        Box::new(
            input
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => input))
                .into_future()
                .and_then({
                    let repo_factory = self.repo_factory.clone();
                    let service = self.clone();
                    move |_| {
                        service.spawn_on_pool(move |conn| {
                            let users_repo = repo_factory.create_users_repo(&conn);

                            users_repo.create(input.clone()).map_err(ectx!(ErrorKind::Internal => input))
                        })
                    }
                }),
        )
    }
    fn get_user(&self, user_id: UserId) -> Box<Future<Item = Option<User>, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo.get(user_id).map_err(ectx!(ErrorKind::Internal => user_id))
        })
    }
    fn update_user(&self, user_id: UserId, payload: UpdateUser) -> Box<Future<Item = User, Error = Error> + Send> {
        Box::new(
            payload
                .validate()
                .map_err(|e| ectx!(err e.clone(), ErrorKind::InvalidInput(e) => payload))
                .into_future()
                .and_then({
                    let repo_factory = self.repo_factory.clone();
                    let service = self.clone();
                    move |_| {
                        service.spawn_on_pool(move |conn| {
                            let users_repo = repo_factory.create_users_repo(&conn);

                            users_repo
                                .update(user_id, payload.clone())
                                .map_err(ectx!(ErrorKind::Internal => user_id, payload))
                        })
                    }
                }),
        )
    }
    fn delete_user(&self, user_id: UserId) -> Box<Future<Item = User, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo.delete(user_id).map_err(ectx!(ErrorKind::Internal => user_id))
        })
    }
}

#[cfg(test)]
pub mod tests {
    extern crate diesel;
    extern crate tokio_core;

    use tokio_core::reactor::Core;

    use super::*;
    use services::tests::create_service;

    #[test]
    fn crud() {
        let mut rt = Core::new().unwrap();
        let service = create_service();
        let new_user = NewUser::default();
        assert!(rt.run(service.create_user(new_user.clone())).is_ok());
        assert!(rt.run(service.get_user(new_user.id)).is_ok());
        let payload = UpdateUser {
            name: Some("test".to_string()),
            authentication_token: None,
        };
        assert!(rt.run(service.update_user(new_user.id, payload)).is_ok());
        assert!(rt.run(service.delete_user(new_user.id)).is_ok());
    }
}
