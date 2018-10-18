use std::cell::RefCell;

use diesel::pg::PgConnection;
use diesel::result::Error as DieselError;
use futures_cpupool::CpuPool;

use super::error::*;
use prelude::*;

thread_local! {
    pub static DB_CONN: RefCell<Option<PgPooledConnection>> = RefCell::new(None)
}

/// One of these methods should be used anytime you use Repo methods.
/// It effectively put a db connection to thread local storage, so that repo can use it.
/// This trait is also responsible for removing unhealthy connections from tls.
/// I.e. it provides guarantees that repo inside DbExecor's method closure will get healthy connection
/// or of not, DbExecutor will heal it next time.
pub trait DbExecutor: Clone + Send + Sync + 'static {
    /// Execute some statements, basically queries
    fn execute<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail;

    /// Execute mutations and queries inside one transaction
    fn execute_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail;

    /// Execute mutations that will be rolled back. This is useful for tests, when you
    /// don't want to pollute your database
    #[cfg(test)]
    fn execute_test_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail;
}

#[derive(Clone)]
pub struct DbExecutorImpl {
    db_pool: PgPool,
    db_thread_pool: CpuPool,
}

impl DbExecutorImpl {
    pub fn new(db_pool: PgPool, db_thread_pool: CpuPool) -> Self {
        Self { db_pool, db_thread_pool }
    }
}

impl DbExecutor for DbExecutorImpl {
    fn execute<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail,
    {
        let db_pool = self.db_pool.clone();
        Box::new(self.db_thread_pool.spawn_fn(move || {
            DB_CONN.with(move |tls_conn_cell| -> Result<T, E> {
                put_connection_into_tls(&db_pool, tls_conn_cell)?;
                f().map_err(move |e| {
                    remove_connection_from_tls_if_broken(tls_conn_cell);
                    e
                })
            })
        }))
    }

    fn execute_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail,
    {
        let db_pool = self.db_pool.clone();
        Box::new(self.db_thread_pool.spawn_fn(move || {
            DB_CONN.with(move |tls_conn_cell| -> Result<T, E> {
                put_connection_into_tls(&db_pool, tls_conn_cell)?;
                let mut err: Option<E> = None;
                let res = {
                    let err_ref = &mut err;
                    with_tls_connection(move |conn| {
                        conn.transaction(|| {
                            f().map_err(|e| {
                                *err_ref = Some(e);
                                DieselError::RollbackTransaction
                            })
                        }).map_err(ectx!(ErrorSource::Diesel, ErrorKind::Internal))
                    })
                };
                res.map_err(|e| {
                    let e: E = err.unwrap_or_else(|| e.into());
                    remove_connection_from_tls_if_broken(tls_conn_cell);
                    e
                })
            })
        }))
    }

    #[cfg(test)]
    fn execute_test_transaction<F, T, E>(&self, f: F) -> Box<Future<Item = T, Error = E> + Send + 'static>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
        E: From<Error> + Fail,
    {
        self.execute_transaction(|| {
            let _ = f()?;
            let e: Error = ErrorKind::Internal.into();
            Err(e.into())
        })
    }
}

/// This method should be called inside repos for obtaining connections from
/// thread local storage
pub fn with_tls_connection<F, T>(f: F) -> Result<T, Error>
where
    F: FnOnce(&PgConnection) -> Result<T, Error>,
{
    DB_CONN.with(|tls_conn_cell| -> Result<T, Error> {
        let maybe_conn = tls_conn_cell.borrow();
        if maybe_conn.is_none() {
            return Err(ectx!(err ErrorKind::Internal, ErrorContext::Connection, ErrorKind::Internal));
        }
        let conn_ref = maybe_conn
            .as_ref()
            .take()
            .ok_or(ectx!(try err ErrorKind::Internal, ErrorContext::Connection, ErrorKind::Internal))?;
        f(conn_ref)
    })
}

/// Checkout connection from db_pool and put it into thead local storage
/// if there is no connection already in thread local storage
fn put_connection_into_tls(db_pool: &PgPool, tls_conn_cell: &RefCell<Option<PgPooledConnection>>) -> Result<(), Error> {
    let mut maybe_conn = tls_conn_cell.borrow_mut();
    if maybe_conn.is_none() {
        match db_pool.get() {
            Ok(conn) => *maybe_conn = Some(conn),
            Err(e) => {
                let e: Error = ectx!(err e, ErrorSource::R2D2, ErrorKind::Internal);
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Check if connection is broken and if so - remove from tls.
/// Select 1 is used for checking connection health, like in Diesel framework
fn remove_connection_from_tls_if_broken(tls_conn_cell: &RefCell<Option<PgPooledConnection>>) {
    let mut maybe_conn = tls_conn_cell.borrow_mut();
    let is_broken = match *maybe_conn {
        Some(ref conn) => conn.execute("SELECT 1").is_err(),
        None => false,
    };
    if is_broken {
        *maybe_conn = None;
    }
}
