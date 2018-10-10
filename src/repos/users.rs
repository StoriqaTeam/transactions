use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::users::dsl::*;

pub trait UsersRepo: Send + Sync + 'static {
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> RepoResult<Option<User>>;
    fn create(&self, payload: NewUser) -> RepoResult<User>;
    fn get(&self, user_id: UserId) -> RepoResult<Option<User>>;
    fn update(&self, user_id: UserId, payload: UpdateUser) -> RepoResult<User>;
    fn delete(&self, user_id: UserId) -> RepoResult<User>;
}

#[derive(Clone, Default)]
pub struct UsersRepoImpl;

impl<'a> UsersRepo for UsersRepoImpl {
    fn find_user_by_authentication_token(&self, token: AuthenticationToken) -> RepoResult<Option<User>> {
        with_tls_connection(|conn| {
            users
                .filter(authentication_token.eq(token))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(ectx!(ErrorKind::Internal))
        })
    }

    fn create(&self, payload: NewUser) -> RepoResult<User> {
        let payload_clone = payload.clone();
        with_tls_connection(|conn| {
            diesel::insert_into(users)
                .values(payload.clone())
                .get_result::<User>(conn)
                .map_err(move |e| {
                    let kind = ErrorKind::from_diesel(&e);
                    ectx!(err e, kind => payload_clone)
                })
        })
    }

    fn get(&self, user_id_arg: UserId) -> RepoResult<Option<User>> {
        with_tls_connection(|conn| {
            users
                .filter(id.eq(user_id_arg))
                .limit(1)
                .get_result(conn)
                .optional()
                .map_err(ectx!(ErrorKind::Internal))
        })
    }
    fn update(&self, user_id_arg: UserId, payload: UpdateUser) -> RepoResult<User> {
        with_tls_connection(|conn| {
            let f = users.filter(id.eq(user_id_arg));
            diesel::update(f)
                .set(payload.clone())
                .get_result(conn)
                .map_err(ectx!(ErrorKind::Internal))
        })
    }
    fn delete(&self, user_id_arg: UserId) -> RepoResult<User> {
        with_tls_connection(|conn| {
            let filtered = users.filter(id.eq(user_id_arg.clone()));
            diesel::delete(filtered).get_result(conn).map_err(ectx!(ErrorKind::Internal))
        })
    }
}

#[cfg(test)]
pub mod tests {
    use diesel::r2d2::ConnectionManager;
    use diesel::PgConnection;
    use futures_cpupool::CpuPool;
    use r2d2;
    use tokio_core::reactor::Core;

    use super::*;
    use config::Config;
    use repos::DbExecutorImpl;

    fn create_executor() -> DbExecutorImpl {
        let config = Config::new().unwrap();
        let manager = ConnectionManager::<PgConnection>::new(config.database.url);
        let db_pool = r2d2::Pool::builder().build(manager).unwrap();
        let cpu_pool = CpuPool::new(1);
        DbExecutorImpl::new(db_pool.clone(), cpu_pool.clone())
    }

    #[ignore]
    #[test]
    fn users_create() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.create(new_user)));
        println!("{:?}", res);
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn users_read() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.get(new_user.id)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn users_update() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = UsersRepoImpl::default();
        let new_user = NewUser::default();

        let payload = UpdateUser {
            name: Some("test".to_string()),
            authentication_token: None,
        };
        let res = core.run(db_executor.execute_test_transaction(move || repo.update(new_user.id, payload)));
        assert!(res.is_ok());
    }

    #[ignore]
    #[test]
    fn users_delete() {
        let mut core = Core::new().unwrap();
        let db_executor = create_executor();
        let repo = UsersRepoImpl::default();
        let new_user = NewUser::default();
        let res = core.run(db_executor.execute_test_transaction(move || repo.delete(new_user.id)));
        assert!(res.is_ok());
    }
}
