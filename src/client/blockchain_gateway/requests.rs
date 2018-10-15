use models::*;

#[derive(Debug, Serialize, Clone)]
pub struct PostTransactoinRequest {
    pub raw: BlockchainTransactionId,
}
