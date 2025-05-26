use crate::config::{BlockConfig, Config, ConnectionConfig, OutputFormat};
use crate::grpc_client::GrpcClient;
use crate::utils::{
    current_timestamp, HotToken, Platform, ProgramIds, TokenInfo,
    TokenMetrics,
};
use anyhow::{Context, Result};
use dashmap::DashMap;
use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::signal;
use yellowstone_grpc_proto::prelude::SubscribeUpdateTransaction;

/// Solana Token Scanner
/// Monitors transactions on Solana for token trading activity and detects spikes
pub struct TokenScanner {
    config: Config,
    client: Arc<RpcClient>,
    grpc_client: Arc<GrpcClient>,
    is_scanning: Arc<AtomicBool>,
    token_metrics: Arc<DashMap<String, TokenMetrics>>,
    last_processed_slot: Arc<AtomicU64>,
    program_ids: ProgramIds,
    output_format: OutputFormat,
}

impl TokenScanner {
    /// Creates a new TokenScanner instance
    pub fn new(config: Config) -> Self {
        let client = Arc::new(RpcClient::new_with_commitment(
            config.solana_rpc_url.clone(),
            ConnectionConfig::commitment_config(),
        ));

        Self {
            grpc_client: Arc::new(GrpcClient::new(
                config.grpc_url.clone(),
                config.tx_cache_size,
            )),
            config,
            client,
            is_scanning: Arc::new(AtomicBool::new(false)),
            token_metrics: Arc::new(DashMap::new()),
            last_processed_slot: Arc::new(AtomicU64::new(0)),
            program_ids: ProgramIds::new(),
            output_format: OutputFormat::default(),
        }
    }

    /// Initializes the scanner by connecting to Solana RPC
    pub async fn initialize(&self) -> Result<()> {
        info!("🚀 Initializing Solana Token Scanner...");
        
        // Test connection and get current slot
        let current_slot = self
            .client
            .get_slot()
            .context("Failed to connect to Solana RPC")?;
        
        self.last_processed_slot.store(current_slot, Ordering::Relaxed);
        
        info!("✅ Connected to Solana RPC: {}", self.config.solana_rpc_url);
        
        // Test gRPC connection
        match self.grpc_client.try_connect().await {
            Ok(true) => {
                info!("✅ gRPC connection available: {}", self.config.grpc_url);
                info!("🔗 Running in hybrid mode (RPC + gRPC)");
            }
            Ok(false) => {
                warn!("⚠️ gRPC connection unavailable: {}", self.config.grpc_url);
                warn!("🔗 Running in RPC-only mode");
            }
            Err(e) => {
                warn!("⚠️ gRPC connection test failed: {}", e);
                warn!("🔗 Running in RPC-only mode");
            }
        }
        
        info!("✅ Starting scan from slot: {}", current_slot);
        info!("📊 Spike detection criteria:");
        info!("   - Volume > {} SOL", self.config.volume_threshold);
        info!("   - Unique buyers ≥ {}", self.config.buyers_threshold);
        info!("   - Token age < {} minutes", self.config.age_threshold_minutes);
        info!("🔍 Scanning for token activity...");

        Ok(())
    }

    /// Starts the continuous scanning loop with hybrid RPC polling and gRPC streaming
    pub async fn start_scanning(&self) -> Result<()> {
        if self.is_scanning.load(Ordering::Relaxed) {
            warn!("Scanner is already running");
            return Ok(());
        }

        self.is_scanning.store(true, Ordering::Relaxed);
        info!("📡 Starting hybrid token scanner (RPC + gRPC)...");

        // Start RPC polling loop
        let scanner_clone = self.clone();
        let rpc_handle = tokio::spawn(async move {
            scanner_clone.scan_loop().await;
        });

        // Start gRPC streaming loop with fallback handling
        let scanner_clone = self.clone();
        let grpc_handle = tokio::spawn(async move {
            match scanner_clone.start_grpc_streaming().await {
                Ok(_) => {
                    info!("✅ gRPC streaming completed");
                }
                Err(e) => {
                    warn!("⚠️ gRPC streaming failed, continuing with RPC-only mode: {}", e);
                    // Continue running - don't exit the application
                }
            }
        });

        // Handle shutdown signals
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("⚠️ Received SIGINT. Shutting down gracefully...");
                self.stop();
            }
            _ = rpc_handle => {
                info!("📡 RPC scanner loop completed");
            }
            _ = grpc_handle => {
                info!("🌐 gRPC streaming completed");
            }
        }

        Ok(())
    }

    /// Main scanning loop for RPC polling
    async fn scan_loop(&self) {
        while self.is_scanning.load(Ordering::Relaxed) {
            if let Err(e) = self.scan_iteration().await {
                error!("❌ Error during scan iteration: {}", e);
            }

            tokio::time::sleep(self.config.scan_interval()).await;
        }
    }

    /// Start gRPC streaming for real-time transactions with fallback handling
    async fn start_grpc_streaming(&self) -> Result<()> {
        let scanner = self.clone();
        
        self.grpc_client
            .start_streaming_with_fallback(move |tx: SubscribeUpdateTransaction| {
                let scanner_clone = scanner.clone();
                async move {
                    scanner_clone.process_grpc_transaction(tx).await
                }
            })
            .await
    }

    /// Single scan iteration
    async fn scan_iteration(&self) -> Result<()> {
        // Scan new blocks
        self.scan_new_blocks().await?;
        
        // Detect spikes
        self.detect_spikes().await?;
        
        // Cleanup old tokens periodically
        self.cleanup_old_tokens().await;

        Ok(())
    }

    /// Scans new blocks since the last processed slot
    async fn scan_new_blocks(&self) -> Result<()> {
        let current_slot = self.client.get_slot()?;
        let last_processed = self.last_processed_slot.load(Ordering::Relaxed);

        if current_slot <= last_processed {
            return Ok(()); // No new blocks to process
        }

        // Process blocks in batches to avoid overwhelming the RPC
        let end_slot = std::cmp::min(
            current_slot,
            last_processed + self.config.max_blocks_to_process,
        );

        for slot in (last_processed + 1)..=end_slot {
            if let Err(e) = self.process_block(slot).await {
                error!("❌ Error processing block {}: {}", slot, e);
                // Continue with next block instead of failing completely
            }
        }

        self.last_processed_slot.store(end_slot, Ordering::Relaxed);
        Ok(())
    }

    /// Processes a single block, analyzing its transactions
    async fn process_block(&self, slot: u64) -> Result<()> {
        let block = match self
            .client
            .get_block_with_config(slot, BlockConfig::rpc_block_config())
        {
            Ok(block) => block,
            Err(e) => {
                warn!("Could not fetch block {}: {}", slot, e);
                return Ok(());
            }
        };

        if let Some(transactions) = block.transactions {
            for tx in transactions {
                // De-duplication: skip if already processed via gRPC
                // Extract signature from the transaction
                let signature = match &tx.transaction {
                    solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
                        ui_tx.signatures.first().cloned()
                    }
                    solana_transaction_status::EncodedTransaction::LegacyBinary(_data) => {
                        // For binary transactions, we'd need to decode the signature
                        // For now, skip de-duplication for binary transactions
                        None
                    }
                    solana_transaction_status::EncodedTransaction::Binary(_data, _) => {
                        // For binary transactions, we'd need to decode the signature
                        // For now, skip de-duplication for binary transactions
                        None
                    }
                    _ => None,
                };

                if let Some(sig) = signature {
                    if self.grpc_client.is_transaction_seen(&sig).await {
                        continue; // Skip already processed transaction
                    }
                }
                
                if let Err(e) = self.process_transaction(&tx, block.block_time).await {
                    error!("Error processing transaction: {}", e);
                    // Continue with next transaction
                }
            }
        }

        Ok(())
    }

    /// Processes a single transaction, looking for token activity
    async fn process_transaction(
        &self,
        tx: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        block_time: Option<i64>,
    ) -> Result<()> {
        // Skip failed transactions
        if tx.meta.as_ref().map_or(true, |meta| meta.err.is_some()) {
            return Ok(());
        }

        // For now, create a mock token for demonstration
        // In a real implementation, you would parse the transaction properly
        if let Some(meta) = &tx.meta {
            // Check if there are any token balance changes
            match &meta.post_token_balances {
                solana_transaction_status::option_serializer::OptionSerializer::Some(post_balances) => {
                    if !post_balances.is_empty() {
                        // Create a mock token for demonstration
                        let mock_token_info = self.create_mock_token_info(meta, block_time);
                        if let Some(token_info) = mock_token_info {
                            let timestamp = block_time.map(|t| (t * 1000) as u64).unwrap_or_else(current_timestamp);
                            self.update_token_metrics(token_info, timestamp).await;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Processes a gRPC transaction from Yellowstone stream
    async fn process_grpc_transaction(&self, tx: SubscribeUpdateTransaction) -> Result<()> {
        // Extract transaction data from gRPC message
        if let Some(transaction) = tx.transaction {
            // Skip failed transactions
            if let Some(meta) = &transaction.meta {
                if meta.err.is_some() {
                    return Ok(());
                }

                // Convert gRPC transaction to token info
                if let Some(token_info) = self.create_token_info_from_grpc(&transaction).await {
                    // Use current timestamp for real-time transactions
                    // In production, you might want to get more accurate block time from the slot
                    let timestamp = current_timestamp();
                    
                    self.update_token_metrics(token_info, timestamp).await;
                }
            }
        }

        Ok(())
    }

    /// Creates token info from gRPC transaction data
    async fn create_token_info_from_grpc(
        &self,
        transaction: &yellowstone_grpc_proto::prelude::SubscribeUpdateTransactionInfo,
    ) -> Option<TokenInfo> {
        // This is a simplified implementation for demonstration
        // In a real scanner, you would parse the transaction instructions properly
        
        if let Some(meta) = &transaction.meta {
            // Check for token balance changes
            if !meta.post_token_balances.is_empty() {
                if let Some(balance) = meta.post_token_balances.first() {
                    if let Ok(token_mint) = balance.mint.parse::<Pubkey>() {
                        // Calculate volume from balance changes
                        let mut volume_lamports = 0u64;
                        for (pre, post) in meta.pre_balances.iter().zip(meta.post_balances.iter()) {
                            if pre > post {
                                volume_lamports += pre - post;
                            }
                        }

                        // Use a mock buyer address for now
                        let buyer = Pubkey::new_unique();
                        
                        // Mock platform determination
                        let platform = Platform::PumpFun;

                        return Some(TokenInfo {
                            token_mint,
                            volume: volume_lamports,
                            buyer,
                            platform,
                        });
                    }
                }
            }
        }
        
        None
    }

    /// Creates a mock token info for demonstration purposes
    fn create_mock_token_info(
        &self,
        meta: &solana_transaction_status::UiTransactionStatusMeta,
        _block_time: Option<i64>,
    ) -> Option<TokenInfo> {
        // This is a simplified mock implementation
        // In a real scanner, you would parse the transaction instructions properly
        
        match &meta.post_token_balances {
            solana_transaction_status::option_serializer::OptionSerializer::Some(post_balances) => {
                if let Some(balance) = post_balances.first() {
                    if let Ok(token_mint) = balance.mint.parse::<Pubkey>() {
                        // Calculate volume from balance changes
                        let mut volume_lamports = 0u64;
                        for (pre, post) in meta.pre_balances.iter().zip(meta.post_balances.iter()) {
                            if pre > post {
                                volume_lamports += pre - post;
                            }
                        }

                        // Use a mock buyer address
                        let buyer = Pubkey::new_unique();
                        
                        // Mock platform (in real implementation, determine from program ID)
                        let platform = Platform::PumpFun;

                        return Some(TokenInfo {
                            token_mint,
                            volume: volume_lamports,
                            buyer,
                            platform,
                        });
                    }
                }
            }
            _ => {}
        }
        
        None
    }

    /// Updates the in-memory token metrics map with new token information
    async fn update_token_metrics(&self, token_info: TokenInfo, timestamp: u64) {
        let mint_key = token_info.token_mint.to_string();

        if let Some(mut metrics) = self.token_metrics.get_mut(&mint_key) {
            // Update existing token metrics
            metrics.update(token_info, timestamp);
        } else {
            // New token discovered
            let metrics = TokenMetrics::new(token_info, timestamp);
            self.token_metrics.insert(mint_key, metrics);
        }
    }

    /// Detects spikes in token activity and logs them to console
    async fn detect_spikes(&self) -> Result<()> {
        let current_time = current_timestamp();
        let mut hot_tokens = Vec::new();

        for entry in self.token_metrics.iter() {
            let metrics = entry.value();
            
            // Skip tokens older than threshold
            let age_minutes = metrics.age_minutes(current_time);
            if age_minutes > self.config.age_threshold_minutes {
                continue;
            }

            let volume_in_sol = metrics.volume_in_sol();
            let unique_buyers_count = metrics.unique_buyers_count();

            // Apply spike detection criteria
            if volume_in_sol > self.config.volume_threshold 
                && unique_buyers_count >= self.config.buyers_threshold 
            {
                hot_tokens.push(HotToken::from(metrics));
            }
        }

        // Sort by volume and log the results
        hot_tokens.sort_by(|a, b| b.volume_in_sol.partial_cmp(&a.volume_in_sol).unwrap());
        
        for hot_token in hot_tokens {
            self.log_hot_token(&hot_token);
        }

        Ok(())
    }

    /// Logs information about a hot token to the console
    fn log_hot_token(&self, token: &HotToken) {
        let format = &self.output_format;
        println!(
            "{} {} — {}{:.2} SOL{}{}{}{}{}{} min{}{}{}{} | {}", 
            format.hot_token_prefix,
            token.symbol,
            format.volume_label,
            token.volume_in_sol,
            format.separator,
            format.buyers_label,
            token.unique_buyers_count,
            format.separator,
            format.age_label,
            token.age_minutes,
            format.age_unit,
            format.separator,
            format.platform_label,
            token.platform.as_str(),
            token.mint.to_string()
        );
    }

    /// Removes old tokens from memory to prevent memory leaks
    async fn cleanup_old_tokens(&self) {
        let current_time = current_timestamp();
        let old_threshold_ms = self.config.age_threshold_minutes * 60 * 1000 * self.config.cleanup_age_multiplier;

        self.token_metrics.retain(|_, metrics| {
            current_time - metrics.last_seen <= old_threshold_ms
        });
    }

    /// Stops the scanner
    pub fn stop(&self) {
        self.is_scanning.store(false, Ordering::Relaxed);
        info!("🛑 Scanner stopped.");
    }
}

impl Clone for TokenScanner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            client: Arc::clone(&self.client),
            grpc_client: Arc::clone(&self.grpc_client),
            is_scanning: Arc::clone(&self.is_scanning),
            token_metrics: Arc::clone(&self.token_metrics),
            last_processed_slot: Arc::clone(&self.last_processed_slot),
            program_ids: self.program_ids.clone(),
            output_format: self.output_format.clone(),
        }
    }
} 