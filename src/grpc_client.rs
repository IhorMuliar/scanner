use anyhow::{Context, Result};
use futures::StreamExt;
use log::{error, info, warn};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::{
    subscribe_update::UpdateOneof, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterBlocks, SubscribeRequestFilterTransactions,
};
use solana_sdk::hash::Hash;
use std::str::FromStr;

/// gRPC client for Yellowstone Geyser integration
pub struct GrpcClient {
    grpc_url: String,
    tx_signature_cache: Arc<Mutex<LruCache<String, ()>>>,
    slot_to_blockhash: Arc<Mutex<HashMap<u64, Hash>>>,
}

impl GrpcClient {
    /// Creates a new gRPC client instance
    pub fn new(grpc_url: String, cache_size: usize) -> Self {
        let cache_size = NonZeroUsize::new(cache_size).unwrap_or(NonZeroUsize::new(10000).unwrap());
        
        Self {
            grpc_url,
            tx_signature_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
            slot_to_blockhash: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a transaction signature has been seen before (de-duplication)
    pub async fn is_transaction_seen(&self, signature: &str) -> bool {
        let cache = self.tx_signature_cache.lock().await;
        cache.contains(signature)
    }

    /// Mark a transaction signature as seen
    pub async fn mark_transaction_seen(&self, signature: String) {
        let mut cache = self.tx_signature_cache.lock().await;
        cache.put(signature, ());
    }

    /// Get blockhash for a specific slot
    pub async fn get_blockhash(&self, slot: u64) -> Option<Hash> {
        let blockhash_map = self.slot_to_blockhash.lock().await;
        blockhash_map.get(&slot).copied()
    }

    /// Store blockhash for a slot
    async fn store_blockhash(&self, slot: u64, blockhash: Hash) {
        let mut blockhash_map = self.slot_to_blockhash.lock().await;
        blockhash_map.insert(slot, blockhash);
        
        // Keep only recent slots (last 1000 slots)
        if blockhash_map.len() > 1000 {
            let slots_to_remove: Vec<u64> = blockhash_map
                .keys()
                .filter(|&&s| s < slot.saturating_sub(1000))
                .copied()
                .collect();
            
            for old_slot in slots_to_remove {
                blockhash_map.remove(&old_slot);
            }
        }
    }

    /// Create a subscribe request for monitoring relevant accounts and transactions
    fn create_subscribe_request() -> SubscribeRequest {
        let mut request = SubscribeRequest::default();

        // Subscribe to blocks for blockhash tracking
        request.blocks = HashMap::from([(
            "blocks".to_string(),
            SubscribeRequestFilterBlocks {
                account_include: vec![],
                include_transactions: Some(false),
                include_accounts: Some(false),
                include_entries: Some(false),
            },
        )]);

        // Subscribe to transactions involving token programs and DEX platforms
        request.transactions = HashMap::from([(
            "transactions".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include: vec![
                    // Token Program
                    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                    // Token Program 2022
                    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
                    // System Program (for SOL transfers)
                    "11111111111111111111111111111111".to_string(),
                ],
                account_exclude: vec![],
                account_required: vec![],
            },
        )]);

        // Subscribe to relevant accounts (token accounts, DEX programs)
        request.accounts = HashMap::from([(
            "accounts".to_string(),
            SubscribeRequestFilterAccounts {
                account: vec![],
                owner: vec![
                    // Token Program
                    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
                    // Token Program 2022
                    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".to_string(),
                ],
                filters: vec![],
                nonempty_txn_signature: None,
            },
        )]);

        request
    }

    /// Start the gRPC streaming loop
    pub async fn start_streaming<F, Fut>(&self, mut on_transaction: F) -> Result<()>
    where
        F: FnMut(yellowstone_grpc_proto::prelude::SubscribeUpdateTransaction) -> Fut + Send,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        info!("🌐 Connecting to Yellowstone gRPC at: {}", self.grpc_url);

        let mut client = GeyserGrpcClient::build_from_shared(self.grpc_url.clone())
            .context("Failed to build gRPC client")?
            .connect()
            .await
            .context("Failed to connect to gRPC server")?;

        let subscribe_request = Self::create_subscribe_request();
        
        info!("📡 Starting gRPC subscription...");
        
        let (_, mut stream) = client
            .subscribe_with_request(Some(subscribe_request))
            .await
            .context("Failed to subscribe to gRPC stream")?;

        info!("✅ gRPC stream established successfully");

        while let Some(message) = stream.next().await {
            match message {
                Ok(msg) => {
                    if let Err(e) = self.handle_stream_message(msg, &mut on_transaction).await {
                        error!("❌ Error handling stream message: {}", e);
                    }
                }
                Err(e) => {
                    error!("❌ gRPC stream error: {}", e);
                    // Optionally implement reconnection logic here
                    break;
                }
            }
        }

        warn!("🔌 gRPC stream ended");
        Ok(())
    }

    /// Handle incoming stream messages
    async fn handle_stream_message<F, Fut>(
        &self,
        message: yellowstone_grpc_proto::prelude::SubscribeUpdate,
        on_transaction: &mut F,
    ) -> Result<()>
    where
        F: FnMut(yellowstone_grpc_proto::prelude::SubscribeUpdateTransaction) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        match message.update_oneof {
            Some(UpdateOneof::Block(block)) => {
                // Store blockhash for later use
                if let Ok(blockhash) = Hash::from_str(&block.blockhash) {
                    self.store_blockhash(block.slot, blockhash).await;
                }
                // Log block updates occasionally
                if block.slot % 1000 == 0 {
                    info!("📦 Block update: slot {} hash {}", block.slot, block.blockhash);
                }
            }
            Some(UpdateOneof::Transaction(tx)) => {
                // Check for de-duplication
                if let Some(transaction) = &tx.transaction {
                    if !transaction.signature.is_empty() {
                        let signature = bs58::encode(&transaction.signature).into_string();
                        if self.is_transaction_seen(&signature).await {
                            // Skip already processed transaction
                            return Ok(());
                        }
                        
                        // Mark as seen for de-duplication
                        self.mark_transaction_seen(signature).await;
                    }
                }
                
                // Process the transaction
                on_transaction(tx).await?;
            }
            Some(UpdateOneof::Account(_account)) => {
                // Handle account updates if needed in the future
            }
            _ => {
                // Handle other message types if needed
            }
        }

        Ok(())
    }
} 