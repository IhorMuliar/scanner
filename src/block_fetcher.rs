use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::slot_history::Slot;
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_transaction_status::{UiConfirmedBlock, UiTransactionEncoding, TransactionDetails};
use log::{debug, warn};

pub struct BlockFetcher {
    rpc_client: RpcClient,
}

impl BlockFetcher {
    pub fn new(rpc_url: &str) -> Result<Self> {
        let rpc_client = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(), // Use confirmed for faster processing
        );
        
        Ok(Self { rpc_client })
    }
    
    pub async fn get_latest_slot(&self) -> Result<Slot> {
        let slot = self.rpc_client.get_slot()?;
        debug!("Latest slot: {}", slot);
        Ok(slot)
    }
    
    pub async fn get_block(&self, slot: Slot) -> Result<Option<UiConfirmedBlock>> {
        let config = RpcBlockConfig {
            encoding: Some(UiTransactionEncoding::Json),
            transaction_details: Some(TransactionDetails::Full),
            rewards: Some(false),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };
        
        match self.rpc_client.get_block_with_config(slot, config) {
            Ok(block) => {
                debug!("Successfully fetched block for slot {}", slot);
                Ok(Some(block))
            },
            Err(e) => {
                // Some slots might be skipped, this is normal
                if e.to_string().contains("was skipped") || e.to_string().contains("not available") {
                    debug!("Slot {} was skipped or not available", slot);
                    Ok(None)
                } else {
                    warn!("Error fetching block for slot {}: {}", slot, e);
                    Err(e.into())
                }
            }
        }
    }
} 