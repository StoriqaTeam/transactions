use uuid::Uuid;

use models::*;

#[derive(Debug, Deserialize, Clone)]
pub struct CreateAccountAddressResponse {
    pub id: Uuid,
    pub currency: Currency,
    pub address: AccountAddress,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreateBlockchainTxResponse {
    #[serde(flatten)]
    pub blockchain_tx: BlockchainTransactionRaw,
}
