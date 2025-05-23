use crate::config::{Config, ConnectionConfig, OutputFormat};
use crate::types::{HotToken, ProgramIds, TokenInfo, TokenMetrics};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{EncodedTransaction, UiTransactionEncoding};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// Main token scanner that monitors Solana blockchain for token activity
pub struct TokenScanner {
    rpc_client: Arc<RpcClient>,
    config: Config,
    connection_config: ConnectionConfig,
    output_format: OutputFormat,
    token_metrics: Arc<DashMap<Pubkey, TokenMetrics>>,
    last_processed_slot: Arc<AtomicU64>,
    is_scanning: Arc<AtomicBool>,
    program_ids: Vec<Pubkey>,
}

impl TokenScanner {
    /// Creates a new TokenScanner instance
    pub async fn new() -> Result<Self> {
        let config = Config::load();
        let connection_config = ConnectionConfig::default();
        let output_format = OutputFormat::default();

        info!("🚀 Initializing Solana Token Scanner...");
        
        // Create RPC client
        let rpc_client = Arc::new(
            RpcClient::new_with_timeout_and_commitment(
                config.solana_rpc_url.clone(),
                connection_config.timeout,
                connection_config.commitment,
            )
        );

        info!("✅ Connected to Solana RPC: {}", config.solana_rpc_url);

        // Get current slot to start scanning from
        let current_slot = rpc_client
            .get_slot()
            .context("Failed to get current slot")?;
        
        info!("✅ Starting scan from slot: {}", current_slot);

        // Log spike detection criteria
        info!("📊 Spike detection criteria:");
        info!("   - Volume > {} SOL", config.volume_threshold);
        info!("   - Unique buyers ≥ {}", config.buyers_threshold);
        info!("   - Token age < {} minutes", config.age_threshold_minutes);
        info!("");
        info!("🔍 Scanning for token activity...");

        let program_ids = ProgramIds::get_all();

        Ok(Self {
            rpc_client,
            config,
            connection_config,
            output_format,
            token_metrics: Arc::new(DashMap::new()),
            last_processed_slot: Arc::new(AtomicU64::new(current_slot)),
            is_scanning: Arc::new(AtomicBool::new(false)),
            program_ids,
        })
    }

    /// Starts the continuous scanning loop
    pub async fn start_scanning(&mut self) -> Result<()> {
        if self.is_scanning.load(Ordering::Acquire) {
            return Ok(());
        }

        self.is_scanning.store(true, Ordering::Release);

        // Start cleanup task
        let cleanup_task = self.start_cleanup_task();

        // Main scanning loop
        let scan_task = self.run_scan_loop();

        // Run both tasks concurrently
        tokio::select! {
            result = scan_task => {
                if let Err(e) = result {
                    error!("Scan loop error: {}", e);
                }
            }
            result = cleanup_task => {
                if let Err(e) = result {
                    error!("Cleanup task error: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Main scanning loop that processes new blocks
    async fn run_scan_loop(&self) -> Result<()> {
        let mut scan_interval = interval(self.config.scan_interval());
        scan_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        while self.is_scanning.load(Ordering::Acquire) {
            scan_interval.tick().await;
            
            let start_time = Instant::now();
            
            match self.scan_new_blocks().await {
                Ok(blocks_processed) => {
                    if blocks_processed > 0 {
                        debug!("Processed {} blocks in {:?}", blocks_processed, start_time.elapsed());
                    }
                }
                Err(e) => {
                    error!("Error during block scan: {}", e);
                }
            }

            // Detect and log hot tokens
            if let Err(e) = self.detect_and_log_spikes().await {
                error!("Error during spike detection: {}", e);
            }
        }

        Ok(())
    }

    /// Scans new blocks since the last processed slot
    async fn scan_new_blocks(&self) -> Result<u64> {
        let current_slot = self.rpc_client
            .get_slot()
            .context("Failed to get current slot")?;

        let last_processed = self.last_processed_slot.load(Ordering::Acquire);

        if current_slot <= last_processed {
            return Ok(0); // No new blocks to process
        }

        // Process blocks in batches to avoid overwhelming the RPC
        let end_slot = std::cmp::min(
            current_slot,
            last_processed + self.config.max_blocks_to_process,
        );

        let mut blocks_processed = 0;

        for slot in (last_processed + 1)..=end_slot {
            match self.process_block(slot).await {
                Ok(_) => blocks_processed += 1,
                Err(e) => {
                    warn!("Failed to process block {}: {}", slot, e);
                    // Continue processing other blocks despite individual failures
                }
            }
        }

        self.last_processed_slot.store(end_slot, Ordering::Release);

        Ok(blocks_processed)
    }

    /// Processes a single block, analyzing its transactions
    async fn process_block(&self, slot: u64) -> Result<()> {
        let block = self
            .rpc_client
            .get_block_with_config(
                slot,
                solana_client::rpc_config::RpcBlockConfig {
                    encoding: Some(UiTransactionEncoding::JsonParsed),
                    transaction_details: Some(
                        solana_transaction_status::TransactionDetails::Full,
                    ),
                    rewards: Some(false),
                    commitment: Some(self.connection_config.commitment),
                    max_supported_transaction_version: Some(0),
                },
            )
            .context(format!("Failed to get block {}", slot))?;

        let block_time = block
            .block_time
            .map(|t| DateTime::from_timestamp(t, 0).unwrap_or_else(Utc::now))
            .unwrap_or_else(Utc::now);

        if let Some(transactions) = block.transactions {
            for tx in transactions {
                if let Err(e) = self.process_transaction(&tx, block_time).await {
                    debug!("Error processing transaction: {}", e);
                    // Continue processing other transactions
                }
            }
        }

        Ok(())
    }

    /// Processes a single transaction, looking for token activity
    async fn process_transaction(
        &self,
        tx: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        block_time: DateTime<Utc>,
    ) -> Result<()> {
        // Skip failed transactions or those without metadata
        if let Some(meta) = &tx.meta {
            if meta.err.is_some() {
                return Ok(());
            }
        } else {
            return Ok(());
        }

        // Extract transaction message
        let transaction = match &tx.transaction {
            EncodedTransaction::Json(ui_tx) => ui_tx,
            _ => return Ok(()), // Skip non-JSON encoded transactions
        };

        // Use a simpler approach to check if any of our target programs are involved
        // Extract all account keys from the transaction as strings
        let account_keys = match serde_json::to_value(&transaction.message) {
            Ok(message_value) => {
                // Try to extract account keys from the message JSON
                let account_keys_value = message_value
                    .get("accountKeys")
                    .or_else(|| message_value.get("account_keys"));
                
                if let Some(keys_array) = account_keys_value.and_then(|v| v.as_array()) {
                    // Extract pubkeys from the array
                    keys_array
                        .iter()
                        .filter_map(|key_value| {
                            // Handle both simple string keys and objects with pubkey field
                            if let Some(key_str) = key_value.as_str() {
                                Some(key_str.to_string())
                            } else if let Some(obj) = key_value.as_object() {
                                obj.get("pubkey").and_then(|k| k.as_str()).map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<String>>()
                } else {
                    return Ok(());
                }
            }
            Err(_) => return Ok(()),
        };

        // Check if any of our target programs are involved
        let mut relevant_program_index = None;
        let program_ids_str: Vec<String> = self.program_ids
            .iter()
            .map(|id| id.to_string())
            .collect();

        for (index, key) in account_keys.iter().enumerate() {
            if program_ids_str.contains(key) {
                relevant_program_index = Some(index);
                break;
            }
        }

        if let Some(program_index) = relevant_program_index {
            // Extract token information from the transaction
            if let Some(token_info) = self.extract_token_info(tx, program_index, &account_keys)? {
                self.update_token_metrics(token_info, block_time);
            }
        }

        Ok(())
    }

    /// Extracts token information from a transaction
    fn extract_token_info(
        &self,
        tx: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        program_index: usize,
        account_keys: &[String],
    ) -> Result<Option<TokenInfo>> {
        // Get the program ID
        let program_id_str = &account_keys[program_index];
        let program_id = program_id_str.parse::<Pubkey>()?;
        let platform = ProgramIds::determine_platform(&program_id);

        // Extract token mint address from post token balances
        let token_mint = if let Some(meta) = &tx.meta {
            // Use serde_json to parse the token balances
            if let Ok(meta_json) = serde_json::to_value(meta) {
                if let Some(balances) = meta_json.get("postTokenBalances").and_then(|v| v.as_array()) {
                    for balance in balances {
                        if let (Some(mint), Some(amount)) = (
                            balance.get("mint").and_then(|m| m.as_str()),
                            balance.get("uiTokenAmount").and_then(|a| a.get("uiAmount")).and_then(|a| a.as_f64()),
                        ) {
                            if amount > 0.0 {
                                if let Ok(mint_pubkey) = mint.parse::<Pubkey>() {
                                    return Ok(Some(TokenInfo {
                                        token_mint: mint_pubkey,
                                        volume: self.extract_volume(meta),
                                        buyer: account_keys[0].parse::<Pubkey>()?,
                                        platform,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            None
        } else {
            None
        };

        // If we couldn't find a token mint, return None
        if token_mint.is_none() {
            return Ok(None);
        }

        Ok(token_mint)
    }

    /// Helper method to extract volume from transaction metadata
    fn extract_volume(&self, meta: &solana_transaction_status::UiTransactionStatusMeta) -> u64 {
        let pre_balances = &meta.pre_balances;
        let post_balances = &meta.post_balances;
        
        if pre_balances.len() == post_balances.len() {
            let mut volume_lamports = 0u64;
            for (pre, post) in pre_balances.iter().zip(post_balances.iter()) {
                if pre > post {
                    volume_lamports += pre - post;
                }
            }
            volume_lamports
        } else {
            0
        }
    }

    /// Updates the in-memory token metrics with new token information
    fn update_token_metrics(&self, token_info: TokenInfo, timestamp: DateTime<Utc>) {
        let token_mint = token_info.token_mint;

        match self.token_metrics.get_mut(&token_mint) {
            Some(mut metrics) => {
                // Update existing token metrics
                metrics.update(token_info, timestamp);
            }
            None => {
                // Create new token metrics
                let metrics = TokenMetrics::new(token_info, timestamp);
                self.token_metrics.insert(token_mint, metrics);
            }
        }
    }

    /// Detects spikes in token activity and logs them to console
    async fn detect_and_log_spikes(&self) -> Result<()> {
        let mut hot_tokens = Vec::new();

        // Collect hot tokens based on criteria
        for item in self.token_metrics.iter() {
            let metrics = item.value();
            
            // Skip tokens older than threshold
            if metrics.age_minutes() > self.config.age_threshold_minutes as i64 {
                continue;
            }

            let volume_sol = metrics.volume_in_sol();
            let unique_buyers_count = metrics.unique_buyers_count();

            // Apply spike detection criteria
            if volume_sol > self.config.volume_threshold 
                && unique_buyers_count >= self.config.buyers_threshold 
            {
                hot_tokens.push(HotToken::from(metrics));
            }
        }

        // Sort by volume and log the results
        hot_tokens.sort_by(|a, b| b.volume_sol.partial_cmp(&a.volume_sol).unwrap());

        for token in hot_tokens {
            self.log_hot_token(&token);
        }

        Ok(())
    }

    /// Logs information about a hot token to the console
    fn log_hot_token(&self, token: &HotToken) {
        println!(
            "{} {} — {}:{:.2} SOL{}{}{}{}{}{} min{}{}{} {}",
            self.output_format.hot_token_prefix,
            token.symbol,
            self.output_format.volume_label,
            token.volume_sol,
            self.output_format.separator,
            self.output_format.buyers_label,
            token.unique_buyers_count,
            self.output_format.separator,
            self.output_format.age_label,
            token.age_minutes,
            self.output_format.age_unit,
            self.output_format.separator,
            self.output_format.platform_label,
            token.platform
        );
    }

    /// Starts the cleanup task that removes old tokens from memory
    async fn start_cleanup_task(&self) -> Result<()> {
        let mut cleanup_interval = interval(self.config.cleanup_interval());
        cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let token_metrics = Arc::clone(&self.token_metrics);
        let config = self.config.clone();
        let is_scanning = Arc::clone(&self.is_scanning);

        tokio::spawn(async move {
            while is_scanning.load(Ordering::Acquire) {
                cleanup_interval.tick().await;
                
                let now = Utc::now();
                let cleanup_threshold_minutes = 
                    config.age_threshold_minutes * config.cleanup_age_multiplier;

                let mut removed_count = 0;
                
                token_metrics.retain(|_, metrics| {
                    let age_minutes = (now - metrics.last_seen).num_minutes();
                    let should_keep = age_minutes <= cleanup_threshold_minutes as i64;
                    if !should_keep {
                        removed_count += 1;
                    }
                    should_keep
                });

                if removed_count > 0 {
                    debug!("Cleaned up {} old tokens from memory", removed_count);
                }
            }
        });

        Ok(())
    }

    /// Stops the scanner gracefully
    pub async fn stop(&mut self) {
        self.is_scanning.store(false, Ordering::Release);
        info!("🛑 Scanner stopped.");
    }
} 