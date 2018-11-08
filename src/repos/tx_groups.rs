use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::tx_groups::dsl::*;

pub trait TxGroupsRepo: Send + Sync + 'static {
    fn create(&self, payload: NewTxGroup) -> RepoResult<TxGroup>;
    fn get(&self, id: TxGroupId) -> RepoResult<Option<TxGroup>>;
}

#[derive(Clone, Default)]
pub struct TxGroupsRepoImpl;

impl TxGroupsRepo for TxGroupsRepoImpl {
    fn create(&self, payload: NewTxGroup) -> RepoResult<TxGroup> {
        with_tls_connection(|conn| {
            diesel::insert_into(tx_groups)
                .values(payload.clone())
                .get_result::<TxGroup>(conn)
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => payload)
                })
        })
    }

    fn get(&self, id_: TxGroupId) -> RepoResult<Option<TxGroup>> {
        with_tls_connection(|conn| {
            tx_groups.filter(id.eq(id_)).limit(1).get_result(conn).optional().map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => id_)
            })
        })
    }
}
