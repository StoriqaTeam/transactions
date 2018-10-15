use models::*;

#[derive(Debug, Deserialize, Clone)]
pub struct CreateAccountAddressResponse {
    pub account_address: AccountAddress,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreateBlockchainTxResponse {
    #[serde(flatten)]
    pub blockchain_tx_id: BlockchainTransactionId,
}
