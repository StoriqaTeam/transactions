use models::*;

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetEtheriumNonceResponse {
    pub nonce: u64,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetBitcoinUtxosResponse {
    #[serde(flatten)]
    pub utxos: Vec<BitcoinUtxos>,
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
