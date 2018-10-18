use diesel::sql_types::Varchar;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, PartialEq)]
#[sql_type = "Varchar"]
pub struct BlockchainTransactionId(String);
derive_newtype_sql!(blockchain_transaction_id, Varchar, BlockchainTransactionId, BlockchainTransactionId);

impl BlockchainTransactionId {
    pub fn new(id: String) -> Self {
        BlockchainTransactionId(id)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Default for BlockchainTransactionId {
    fn default() -> Self {
        BlockchainTransactionId(Uuid::new_v4().to_string())
    }
}
