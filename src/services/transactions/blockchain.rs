use std::sync::Arc;

use super::super::error::*;
use client::{BlockchainClient, KeysClient};
use models::*;
use prelude::*;
use repos::PendingBlockchainTransactionsRepo;
use utils::log_and_capture_error;

pub trait BlockchainService: Send + Sync + 'static {
    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee: Amount,
    ) -> Result<BlockchainTransactionId, Error>;
    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Result<BlockchainTransactionId, Error>;
}

#[derive(Clone)]
pub struct BlockchainServiceImpl {
    keys_client: Arc<dyn KeysClient>,
    blockchain_client: Arc<dyn BlockchainClient>,
    pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
}

impl BlockchainServiceImpl {
    pub fn new(
        keys_client: Arc<dyn KeysClient>,
        blockchain_client: Arc<dyn BlockchainClient>,
        pending_blockchain_transactions_repo: Arc<PendingBlockchainTransactionsRepo>,
    ) -> Self {
        Self {
            keys_client,
            blockchain_client,
            pending_blockchain_transactions_repo,
        }
    }
}

impl BlockchainService for BlockchainServiceImpl {
    fn create_bitcoin_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee: Amount,
    ) -> Result<BlockchainTransactionId, Error> {
        let from_clone = from.clone();
        let utxos = self
            .blockchain_client
            .get_bitcoin_utxos(from.clone())
            .map_err(ectx!(try convert => from_clone))
            .wait()?;

        let create_blockchain_input = CreateBlockchainTx::new(from, to, Currency::Btc, value, fee, None, Some(utxos));
        let create_blockchain_input_clone = create_blockchain_input.clone();

        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone(), Role::User)
            .map_err(ectx!(try convert => create_blockchain_input_clone))
            .wait()?;

        let blockchain_tx_id = self
            .blockchain_client
            .post_bitcoin_transaction(raw_tx)
            .map_err(ectx!(try convert))
            .wait()?;

        let new_pending = (create_blockchain_input, blockchain_tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_blockchain_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };

        Ok(blockchain_tx_id)
    }

    fn create_ethereum_tx(
        &self,
        from: BlockchainAddress,
        to: BlockchainAddress,
        value: Amount,
        fee: Amount,
        currency: Currency,
    ) -> Result<BlockchainTransactionId, Error> {
        match currency {
            Currency::Eth => (),
            Currency::Stq => (),
            _ => return Err(ectx!(err ErrorContext::InvalidCurrency, ErrorKind::Internal)),
        }
        let from_clone = from.clone();
        let nonce = self
            .blockchain_client
            .get_ethereum_nonce(from_clone.clone())
            .map_err(ectx!(try convert => from_clone))
            .wait()?;

        // creating blockchain transactions array
        let create_blockchain_input = CreateBlockchainTx::new(from, to, currency, value, fee, Some(nonce), None);

        let create_blockchain = create_blockchain_input.clone();
        let raw_tx = self
            .keys_client
            .sign_transaction(create_blockchain_input.clone(), Role::User)
            .map_err(ectx!(try convert => create_blockchain_input))
            .wait()?;
        let tx_id = self
            .blockchain_client
            .post_ethereum_transaction(raw_tx)
            .map_err(ectx!(try convert))
            .wait()?;

        let tx_id = match currency {
            Currency::Eth => tx_id,
            // Erc-20 token, we need event log number here, to make a tx_id unique
            _ => BlockchainTransactionId::new(format!("{}:0", tx_id)),
        };
        let new_pending = (create_blockchain, tx_id.clone()).into();
        // Note - we don't rollback here, because the tx is already in blockchain. so after that just silently
        // fail if we couldn't write a pending tx. Not having pending tx in db doesn't do a lot of harm, we could cure
        // it later.
        match self.pending_blockchain_transactions_repo.create(new_pending) {
            Err(e) => log_and_capture_error(e),
            _ => (),
        };
        Ok(tx_id)
    }
}
