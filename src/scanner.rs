use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::time::{sleep, Duration, Instant};
use solana_client::rpc_client::RpcClient;
use solana_transaction_status::UiConfirmedBlock;
use log::{info, error};
use anyhow::Result;
use tokio::sync::Mutex;
use std::collections::HashSet;

use crate::config::{Config, get_connection_config, get_block_config};
use crate::utils::{
    ProgramIds, determine_platform, TokenInfo, create_token_info,
    TokenMetricsManager, SpikeDetectionResult, format_spike_output,
    get_current_timestamp
};

/// Solana Token Scanner
/// Monitors transactions on Solana for token trading activity and detects spikes
#[derive(Clone)]
pub struct TokenScanner {
    connection: Arc<RpcClient>,
    is_scanning: Arc<AtomicBool>,
    last_processed_slot: Arc<AtomicU64>,
    token_metrics: Arc<Mutex<TokenMetricsManager>>,
    detected_spikes: Arc<Mutex<HashSet<String>>>,
    last_cleanup: Arc<Mutex<Instant>>,
}

impl TokenScanner {
    /// Creates a new TokenScanner instance
    pub async fn new() -> Result<Self> {
        let connection = Arc::new(RpcClient::new_with_commitment(
            Config::SOLANA_RPC_URL.to_string(),
            get_connection_config(),
        ));

        Ok(Self {
            connection,
            is_scanning: Arc::new(AtomicBool::new(false)),
            last_processed_slot: Arc::new(AtomicU64::new(0)),
            token_metrics: Arc::new(Mutex::new(TokenMetricsManager::new())),
            detected_spikes: Arc::new(Mutex::new(HashSet::new())),
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        })
    }

    /// Initializes the scanner by connecting to Solana RPC
    pub async fn initialize(&mut self) -> Result<()> {
        info!("🚀 Initializing Solana Token Scanner...");
        info!("✅ Connected to Solana RPC: {}", Config::SOLANA_RPC_URL);
        
        // Get current slot to start scanning from
        let current_slot = self.connection.get_slot()?;
        self.last_processed_slot.store(current_slot, Ordering::SeqCst);
        info!("✅ Starting scan from slot: {}", current_slot);
        
        info!("📊 Spike detection criteria:");
        info!("   - Volume > {} SOL", Config::VOLUME_THRESHOLD);
        info!("   - Unique buyers ≥ {}", Config::BUYERS_THRESHOLD);
        info!("   - Token age < {} minutes", Config::AGE_THRESHOLD_MINUTES);
        info!("   - OR: SOL/buys ratio > {} within {} minutes", 
            Config::RATIO_THRESHOLD, Config::RATIO_TIME_WINDOW_MINUTES);
        info!("");
        info!("🔍 Scanning for block activity...");
        
        Ok(())
    }

    /// Starts the continuous scanning loop
    pub async fn start_scanning(&self) -> Result<()> {
        if self.is_scanning.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        self.is_scanning.store(true, Ordering::SeqCst);
        
        while self.is_scanning.load(Ordering::SeqCst) {
            if let Err(e) = self.scan_new_blocks().await {
                error!("❌ Error during scan: {}", e);
                // Continue scanning despite errors
            }
            
            // Perform periodic cleanup
            self.perform_cleanup().await;
            
            sleep(Duration::from_millis(Config::SCAN_INTERVAL_MS)).await;
        }
        
        Ok(())
    }

    /// Scans new blocks since the last processed slot
    async fn scan_new_blocks(&self) -> Result<()> {
        // Get the latest slot
        let current_slot = self.connection.get_slot()?;
        let last_slot = self.last_processed_slot.load(Ordering::SeqCst);
        
        if current_slot <= last_slot {
            return Ok(()); // No new blocks to process
        }
        
        // Process blocks in batches to avoid overwhelming the RPC
        let max_blocks_to_process = Config::MAX_BLOCKS_TO_PROCESS;
        let end_slot = std::cmp::min(current_slot, last_slot + max_blocks_to_process);
        
        info!("📦 Processing blocks {} to {} (current: {})", last_slot + 1, end_slot, current_slot);
        
        for slot in (last_slot + 1)..=end_slot {
            self.process_block(slot).await?;
        }
        
        self.last_processed_slot.store(end_slot, Ordering::SeqCst);
        
        // Check for spikes after processing blocks
        self.detect_and_report_spikes().await;
        
        Ok(())
    }

    /// Processes a single block, analyzing its transactions
    async fn process_block(&self, slot: u64) -> Result<()> {
        match self.connection.get_block_with_config(slot, get_block_config()) {
            Ok(block) => {
                let tx_count = block.transactions.as_ref().map(|txs| txs.len()).unwrap_or(0);
                let block_time = block.block_time.unwrap_or(0);
                
                info!("🧱 Block {}: {} transactions, block_time: {}", slot, tx_count, block_time);
                
                // Process each transaction in the block
                if let Some(transactions) = &block.transactions {
                    for tx in transactions {
                        self.process_transaction(tx, &block).await;
                    }
                }
                
                Ok(())
            }
            Err(e) => {
                error!("❌ Error processing block {}: {}", slot, e);
                // Don't crash on block fetch errors, just log and continue
                Ok(())
            }
        }
    }

    /// Processes a single transaction, looking for token activity
    async fn process_transaction(&self, tx: &solana_transaction_status::EncodedTransactionWithStatusMeta, _block: &UiConfirmedBlock) {
        // Check if transaction has metadata and didn't error
        let _meta = match &tx.meta {
            Some(meta) if meta.err.is_none() => meta,
            _ => return,
        };

        // Extract account keys from the transaction - simplified approach
        let account_keys = match &tx.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
                match &ui_tx.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_msg) => {
                        parsed_msg.account_keys.iter().map(|key| key.pubkey.clone()).collect::<Vec<String>>()
                    }
                    solana_transaction_status::UiMessage::Raw(raw_msg) => {
                        raw_msg.account_keys.clone()
                    }
                }
            }
            solana_transaction_status::EncodedTransaction::LegacyBinary(_) => return,
            solana_transaction_status::EncodedTransaction::Binary(_, _) => return,
            solana_transaction_status::EncodedTransaction::Accounts(_) => return,
        };

        // Check if transaction involves any of our target programs
        let program_ids = vec![
            ProgramIds::PUMP_FUN,
            // ProgramIds::RAYDIUM_AMM,
            // ProgramIds::METEORA,
        ];

        let relevant_program_index = account_keys.iter().position(|key| program_ids.contains(&key.as_str()));
        
        if let Some(program_index) = relevant_program_index {
            // Extract token information from the transaction
            if let Some(token_info) = self.extract_token_info(tx, program_index, &account_keys) {
                // Update token metrics
                self.update_token_metrics(token_info).await;
            }
        }
    }

    /// Extracts token information from a transaction
    fn extract_token_info(&self, tx: &solana_transaction_status::EncodedTransactionWithStatusMeta, program_index: usize, account_keys: &[String]) -> Option<TokenInfo> {
        // let _account_keys = match &tx.transaction {
        //     solana_transaction_status::EncodedTransaction::Json(ui_tx) => {
        //         info!("🔍 Processing transaction {:?}", ui_tx.signatures);

        //         match &ui_tx.message {
        //             solana_transaction_status::UiMessage::Parsed(parsed_msg) => {
        //                 // info!("🔍 Processing parsed instructions {:?}", parsed_msg.instructions);

        //                 for instruction in &parsed_msg.instructions {
        //                     match instruction {
        //                         solana_transaction_status::UiInstruction::Parsed(parsed_instruction) => {
        //                             // info!("🔍 Processing parsed instruction");

        //                             match &parsed_instruction {
        //                                 solana_transaction_status::UiParsedInstruction::Parsed(parsed_instruction) => {
        //                                     info!("🔍 Processing parsed instruction {:?}", parsed_instruction);
        //                                 }
        //                                 solana_transaction_status::UiParsedInstruction::PartiallyDecoded(partially_decoded_instruction) => {
        //                                     info!("🔍 Processing partially decoded instruction {:?}", partially_decoded_instruction);
        //                                 }
        //                             }
        //                         }
        //                         solana_transaction_status::UiInstruction::Compiled(_) => {
        //                             // info!("🔍 Processing compiled instruction");
        //                         }
        //                     }
        //                 }
        //             }
        //             solana_transaction_status::UiMessage::Raw(raw_msg) => {
        //                 info!("🔍 Processing raw instructions {:?}", raw_msg.instructions);
        //             }
        //         }
        //     }
        //     solana_transaction_status::EncodedTransaction::LegacyBinary(_) => return None,
        //     solana_transaction_status::EncodedTransaction::Binary(_, _) => return None,
        //     solana_transaction_status::EncodedTransaction::Accounts(_) => return None,
        // };
        let meta = tx.meta.as_ref()?;
        
        // Get program ID
        let program_id = account_keys.get(program_index)?.clone();
        
        // Extract token mint address from transaction accounts
        // Look for token mint in the account keys (typically one of the first few accounts)
        let token_mint_address = self.find_token_mint_in_accounts(&account_keys, &program_id)?;

        // Extract volume (in lamports) - simplified estimation
        let mut volume_lamports = 0u64;
        for (pre_balance, post_balance) in meta.pre_balances.iter().zip(meta.post_balances.iter()) {
            if pre_balance > post_balance {
                volume_lamports += pre_balance - post_balance;
            }
        }

        // Only process transactions with meaningful volume
        if volume_lamports == 0 {
            return None;
        }

        // Get buyer address (first account key, simplified)
        let buyer_address = account_keys.get(0)?.clone();
        
        // Determine platform
        let platform = determine_platform(&program_id);

        Some(create_token_info(
            token_mint_address,
            volume_lamports,
            buyer_address,
            platform,
        ))
    }

    /// Finds the token mint address in the transaction accounts
    fn find_token_mint_in_accounts(&self, account_keys: &[String], program_id: &str) -> Option<String> {
        // For different platforms, the token mint is typically at different positions
        match program_id {
            // Pump.fun: token mint is usually the second account
            crate::utils::ProgramIds::PUMP_FUN => {
                account_keys.get(1).cloned()
            }
            // Raydium: token mint is usually one of the first few accounts
            crate::utils::ProgramIds::RAYDIUM_AMM => {
                // Look for a valid token mint in the first few accounts
                for i in 1..std::cmp::min(5, account_keys.len()) {
                    if let Some(account) = account_keys.get(i) {
                        if self.is_likely_token_mint(account) {
                            return Some(account.clone());
                        }
                    }
                }
                None
            }
            // Meteora: similar to Raydium
            crate::utils::ProgramIds::METEORA => {
                for i in 1..std::cmp::min(5, account_keys.len()) {
                    if let Some(account) = account_keys.get(i) {
                        if self.is_likely_token_mint(account) {
                            return Some(account.clone());
                        }
                    }
                }
                None
            }
            // Unknown program: try to find any valid token mint
            _ => {
                for account in account_keys.iter().skip(1) {
                    if self.is_likely_token_mint(account) {
                        return Some(account.clone());
                    }
                }
                None
            }
        }
    }

    /// Checks if an account address is likely a token mint
    fn is_likely_token_mint(&self, address: &str) -> bool {
        // Basic validation: must be a valid Solana public key
        if !crate::utils::is_valid_public_key(address) {
            return false;
        }
        
        // Additional heuristics could be added here:
        // - Check if it's not a known system program
        // - Check if it's not a known token program
        // - Validate against known mint patterns
        
        // For now, just ensure it's a valid public key and not a system program
        !self.is_system_program(address)
    }

    /// Checks if an address is a known system program
    fn is_system_program(&self, address: &str) -> bool {
        matches!(address,
            "11111111111111111111111111111111" |  // System Program
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" |  // Token Program
            "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" |  // Associated Token Program
            "ComputeBudget111111111111111111111111111111" |  // Compute Budget Program
            "Vote111111111111111111111111111111111111111" |  // Vote Program
            "Stake11111111111111111111111111111111111111"    // Stake Program
        )
    }

    /// Updates token metrics with new transaction data
    async fn update_token_metrics(&self, token_info: TokenInfo) {
        let mut metrics_manager = self.token_metrics.lock().await;
        
        metrics_manager.update_token(
            token_info.token_mint.clone(),
            token_info.volume,
            token_info.buyer,
            token_info.platform,
        );
        
        // Log the token activity
        if let Some(token_metrics) = metrics_manager.get_token(&token_info.token_mint) {
            // info!(
            //     "💰 Token activity: {} | Volume: {:.2} SOL | Buyers: {} | Age: {} min | Platform: {}",
            //     token_metrics.symbol,
            //     token_metrics.get_volume_sol(),
            //     token_metrics.get_buyer_count(),
            //     token_metrics.get_age_minutes(),
            //     token_metrics.platform
            // );
        }
    }

    /// Detects and reports token spikes
    async fn detect_and_report_spikes(&self) {
        let metrics_manager = self.token_metrics.lock().await;
        let mut detected_spikes = self.detected_spikes.lock().await;
        
        // Process standard volume/buyer spikes
        // let spike_tokens = metrics_manager.get_spike_tokens(
        //     Config::VOLUME_THRESHOLD,
        //     Config::BUYERS_THRESHOLD,
        //     Config::AGE_THRESHOLD_MINUTES,
        // );
        
        // for token_metrics in spike_tokens {
        //     // Check if this is a new spike (not previously detected)
        //     let is_new_spike = !detected_spikes.contains(&token_metrics.token_mint);
            
        //     if is_new_spike {
        //         detected_spikes.insert(token_metrics.token_mint.clone());
                
        //         let spike_result = SpikeDetectionResult::new(token_metrics.clone(), true);
        //         let output = format_spike_output(&spike_result);
                
        //         // Log the spike with high visibility
        //         info!("{}", output);
                
        //         // Also log to console with special formatting
        //         println!("\n{}", "=".repeat(80));
        //         println!("{}", output);
        //         println!("{}", "=".repeat(80));
        //     }
        // }
        
        // Process ratio-based spikes (SOL/buys ratio)
        let ratio_spike_tokens = metrics_manager.get_ratio_spike_tokens(
            Config::RATIO_THRESHOLD,
            Config::RATIO_TIME_WINDOW_MINUTES,
        );
        
        for (token_metrics, recent_sol, recent_buys) in ratio_spike_tokens {
            // Skip if already detected as a standard spike
            if detected_spikes.contains(&token_metrics.token_mint) {
                continue;
            }
            
            // Mark as detected
            detected_spikes.insert(token_metrics.token_mint.clone());
            
            // Create spike result with ratio information
            let spike_result = SpikeDetectionResult::new(token_metrics.clone(), true)
                .with_ratio_spike(recent_sol, recent_buys);
            
            let output = format_spike_output(&spike_result);
            
            // Log the spike with high visibility
            // info!("{}", output);
            
            // // Also log to console with special formatting
            // println!("\n{}", "=".repeat(80));
            // println!("{}", output);
            // println!("{}", "=".repeat(80));
        }
        
        // Log summary statistics
        let total_tokens = metrics_manager.get_token_count();
        let active_spikes = detected_spikes.len();
        
        if total_tokens > 0 {
            info!("📈 Summary: {} tokens tracked, {} active spikes", total_tokens, active_spikes);
        }
    }

    /// Performs periodic cleanup of old data
    async fn perform_cleanup(&self) {
        let mut last_cleanup = self.last_cleanup.lock().await;
        let now = Instant::now();
        
        if now.duration_since(*last_cleanup).as_millis() >= Config::CLEANUP_INTERVAL_MS as u128 {
            *last_cleanup = now;
            
            let mut metrics_manager = self.token_metrics.lock().await;
            let mut detected_spikes = self.detected_spikes.lock().await;
            
            let cleanup_age = Config::AGE_THRESHOLD_MINUTES * Config::CLEANUP_AGE_MULTIPLIER;
            let initial_token_count = metrics_manager.get_token_count();
            
            // Clean up old tokens
            metrics_manager.cleanup_old_tokens(cleanup_age);
            
            // Clean up old spike records
            let cutoff_time = get_current_timestamp() - (cleanup_age * 60 * 1000);
            detected_spikes.retain(|token_mint| {
                if let Some(token) = metrics_manager.get_token(token_mint) {
                    token.first_seen >= cutoff_time
                } else {
                    false // Remove if token no longer exists
                }
            });
            
            let final_token_count = metrics_manager.get_token_count();
            let cleaned_count = initial_token_count.saturating_sub(final_token_count);
            
            if cleaned_count > 0 {
                info!("🧹 Cleanup: removed {} old tokens, {} tokens remaining", cleaned_count, final_token_count);
            }
        }
    }

    /// Stops the scanner
    pub async fn stop(&self) {
        self.is_scanning.store(false, Ordering::SeqCst);
        info!("🛑 Scanner stopped.");
        
        // Final summary
        let metrics_manager = self.token_metrics.lock().await;
        let detected_spikes = self.detected_spikes.lock().await;
        
        info!("📊 Final summary:");
        info!("   - Total tokens tracked: {}", metrics_manager.get_token_count());
        info!("   - Total spikes detected: {}", detected_spikes.len());
    }
} 