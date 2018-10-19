use std::time::SystemTime;

use models::*;
use schema::seen_hashes;

#[derive(Debug, Queryable, Clone)]
pub struct SeenHashes {
    pub hash: BlockchainTransactionId,
    pub block_number: i64,
    pub currency: Currency,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

impl From<BlockchainTransaction> for NewSeenHashes {
    fn from(transaction: BlockchainTransaction) -> Self {
        Self {
            hash: transaction.hash,
            block_number: transaction.block_number as i64,
            currency: transaction.currency,
        }
    }
}

#[derive(Debug, Insertable, Clone)]
#[table_name = "seen_hashes"]
pub struct NewSeenHashes {
    pub hash: BlockchainTransactionId,
    pub block_number: i64,
    pub currency: Currency,
}

impl Default for NewSeenHashes {
    fn default() -> Self {
        Self {
            hash: BlockchainTransactionId::default(),
            block_number: 0,
            currency: Currency::Eth,
        }
    }
}
