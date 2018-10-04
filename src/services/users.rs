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

impl<
        T: Connection<Backend = Pg, TransactionManager = AnsiTransactionManager> + 'static,
        M: ManageConnection<Connection = T>,
        F: ReposFactory<T>,
    > UsersService for Service<T, M, F>
{
    fn create_user(&self, input: NewUser) -> Box<Future<Item = User, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo.create(input.clone()).map_err(ectx!(ErrorKind::Internal => input))
        })
    }
    fn get_user(&self, user_id: UserId) -> Box<Future<Item = Option<User>, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo.get(user_id).map_err(ectx!(ErrorKind::Internal => user_id))
        })
    }
    fn update_user(&self, user_id: UserId, payload: UpdateUser) -> Box<Future<Item = User, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo
                .update(user_id, payload.clone())
                .map_err(ectx!(ErrorKind::Internal => user_id, payload))
        })
    }
    fn delete_user(&self, user_id: UserId) -> Box<Future<Item = User, Error = Error> + Send> {
        let repo_factory = self.repo_factory.clone();
        self.spawn_on_pool(move |conn| {
            let users_repo = repo_factory.create_users_repo(&conn);
            users_repo.delete(user_id).map_err(ectx!(ErrorKind::Internal => user_id))
        })
    }
}
