use diesel::sql_types::Varchar;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromSqlRow, AsExpression, Clone, PartialEq)]
#[sql_type = "Varchar"]
pub struct BlockchainTransactionRaw(String);
derive_newtype_sql!(blockchain_transaction, Varchar, BlockchainTransactionRaw, BlockchainTransactionRaw);

impl BlockchainTransactionRaw {
    pub fn new(raw: String) -> Self {
        BlockchainTransactionRaw(raw)
    }

    pub fn inner(&self) -> &str {
        &self.0
    }
}

impl Default for BlockchainTransactionRaw {
    fn default() -> Self {
        BlockchainTransactionRaw(Uuid::new_v4().to_string())
    }
}
