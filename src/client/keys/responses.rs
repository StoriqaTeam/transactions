use uuid::Uuid;

use models::*;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountAddressResponse {
    pub id: Uuid,
    pub currency: Currency,
    pub blockchain_address: BlockchainAddress,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockchainTxResponse {
    pub raw: BlockchainTransactionRaw,
}
