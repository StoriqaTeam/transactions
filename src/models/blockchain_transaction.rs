use diesel::sql_types::Varchar;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, PartialEq)]
#[sql_type = "Varchar"]
pub struct BlockchainTransaction(String);
derive_newtype_sql!(blockchain_transaction, Varchar, BlockchainTransaction, BlockchainTransaction);

impl BlockchainTransaction {
    pub fn new(raw: String) -> Self {
        BlockchainTransaction(raw)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Default for BlockchainTransaction {
    fn default() -> Self {
        BlockchainTransaction(Uuid::new_v4().to_string())
    }
}
