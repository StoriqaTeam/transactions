use diesel;

use super::error::*;
use super::executor::with_tls_connection;
use super::*;
use models::*;
use prelude::*;
use schema::key_values::dsl::*;

pub trait KeyValuesRepo: Send + Sync + 'static {
    fn get_nonce(&self, address: BlockchainAddress) -> RepoResult<Option<KeyValue>>;
    fn set_nonce(&self, address: BlockchainAddress, nonce: u64) -> RepoResult<u64>;
}

#[derive(Clone, Default)]
pub struct KeyValuesRepoImpl;

impl KeyValuesRepo for KeyValuesRepoImpl {
    fn get_nonce(&self, address: BlockchainAddress) -> RepoResult<Option<KeyValue>> {
        with_tls_connection(|conn| {
            let key_ = format!("nonce:{}", address);
            key_values.filter(key.eq(key_)).first(conn).optional().map_err(move |e| {
                let error_kind = ErrorKind::from(&e);
                ectx!(err e, error_kind => address)
            })
        })
    }
    fn set_nonce(&self, address: BlockchainAddress, nonce: u64) -> RepoResult<u64> {
        with_tls_connection(|conn| {
            let key_ = format!("nonce:{}", address);
            diesel::insert_into(key_values)
                .values(&NewKeyValue {
                    key: key_,
                    value: json!(nonce),
                })
                .on_conflict(key)
                .do_update()
                .set(value.eq(json!(nonce)))
                .get_result::<KeyValue>(conn)
                .map(|kv| kv.value.as_u64().unwrap())
                .map_err(move |e| {
                    let error_kind = ErrorKind::from(&e);
                    ectx!(err e, error_kind => address, nonce)
                })
        })
    }
}
