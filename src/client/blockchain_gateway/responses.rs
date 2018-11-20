use models::*;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetEtheriumNonceResponse {
    pub nonce: u64,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetBalanceResponse {
    pub balance: Amount,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxHashResponse {
    pub tx_hash: BlockchainTransactionId,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateBlockchainTxRequest {
    pub raw: BlockchainTransactionRaw,
}
