use models::*;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApproveInput {
    pub id: TransactionId,
    pub address: BlockchainAddress,
    pub approve_address: BlockchainAddress,
    pub currency: Currency,
    pub value: Amount,
    pub fee_price: Amount,
    pub nonce: u64,
}
