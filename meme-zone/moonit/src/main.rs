use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use borsh::BorshDeserialize;
use bs58;
use chrono::{DateTime, Utc};
use clap::Parser;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::time::{interval, sleep};

/// Moonit program ID - the main program responsible for token creation and trading
const MOONIT_PROGRAM_ID: &str = "MoonCVVNZFSYkqNXP6bxHLPL6QQJiMagDL3qcqUQTrG-idl";

/// Instruction discriminators for Moonit program instructions
const BUY_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const SELL_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];
const MIGRATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [155, 234, 231, 146, 236, 158, 162, 30];

/// Bonding curve constants for graduation calculations
/// Initial virtual token reserves at the start of the curve (1,073,000,000 * 10^6)
const INITIAL_VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000;
/// Total tokens that can be sold from the curve (793,100,000 * 10^6)
const TOTAL_SELLABLE_TOKENS: u64 = 793_100_000_000_000;
/// SOL threshold for graduation to Raydium (approximately 85 SOL)
const GRADUATION_SOL_THRESHOLD: f64 = 85.0;

/// Graduation progress thresholds for alerting
const GRADUATION_ALERT_THRESHOLDS: &[f64] = &[50.0, 70.0, 80.0, 90.0, 95.0, 99.0];

/// Solana Token Scanner - Monitor blockchain for new blocks and transactions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Solana RPC endpoint URL
    #[arg(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    /// Polling interval in seconds
    #[arg(short, long, default_value_t = 1)]
    interval: u64,
    
    /// Polling interval in milliseconds (overrides interval if specified)
    #[arg(short = 'f', long, default_value_t = 0)]
    interval_ms: u64,

    /// Starting slot number (0 for latest)
    #[arg(short, long, default_value_t = 0)]
    start_slot: u64,

    /// Maximum number of blocks to process (0 for unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_blocks: u64,
}

/// Represents a processed block with metadata
#[derive(Debug, Clone)]
struct ProcessedBlock {
    /// Slot number of the block
    slot: u64,
    /// Block hash
    blockhash: String,
    /// Parent slot
    parent_slot: u64,
    /// Block timestamp
    timestamp: Option<DateTime<Utc>>,
    /// Number of transactions in block
    transaction_count: usize,
    /// Processing timestamp
    processed_at: DateTime<Utc>,
}

/// Represents instruction type for a Pump.fun transaction
#[derive(Debug, Clone, PartialEq)]
enum PumpFunInstructionType {
    Buy,
    Sell,
    Create,
    Migrate,
    Other,
}

/// Tracks token buy activity - simplified for logging only
#[derive(Debug)]
struct TokenTracker {
}

impl TokenTracker {
    /// Create a new simplified token tracker
    pub fn new() -> Self {
        info!("Initializing simplified token tracker for logging");
        
        Self {
        }
    }
    
    /// Record a new transaction for logging purposes only
    pub fn record_transaction(&self, mint: String, signature: String, sol_amount: f64, timestamp: DateTime<Utc>, transaction_type: PumpFunInstructionType) {
        debug!("Recording {} transaction for token {}: {:.4} SOL", 
               format!("{:?}", transaction_type), mint, sol_amount);
        
        // Log the transaction details
        match transaction_type {
            PumpFunInstructionType::Buy | PumpFunInstructionType::Create => {
                info!("💰 {} Transaction: {}", format!("{:?}", transaction_type), mint);
                info!("  Signature: {}", signature);
                if sol_amount > 0.0 {
                    info!("  SOL amount: {:.4}", sol_amount);
                }
                info!("  Timestamp: {}", timestamp);
            }
            PumpFunInstructionType::Migrate => {
                info!("🚀 Migration Transaction: {}", mint);
                info!("  Signature: {}", signature);
                info!("  Timestamp: {}", timestamp);
            }
            PumpFunInstructionType::Sell => {
                info!("💸 Sell Transaction: {}", mint);
                info!("  Signature: {}", signature);
                if sol_amount > 0.0 {
                    info!("  SOL amount: {:.4}", sol_amount);
                }
                info!("  Timestamp: {}", timestamp);
            }
            _ => {
                debug!("Other transaction type for token {}", mint);
            }
        }
    }
}

/// Represents a processed transaction with detailed information
#[derive(Debug, Clone)]
struct ProcessedTransaction {
    /// Transaction signature (unique identifier)
    signature: String,
    /// Slot number where transaction was included
    slot: u64,
    /// Block hash where transaction was included
    block_hash: String,
    /// Transaction success status
    is_successful: bool,
    /// Error message if transaction failed
    error_message: Option<String>,
    /// Number of instructions in the transaction
    instruction_count: usize,
    /// Compute units consumed by the transaction
    compute_units_consumed: Option<u64>,
    /// Fee paid for the transaction (in lamports)
    fee: u64,
    /// Accounts involved in the transaction
    account_keys: Vec<String>,
    /// Recent blockhash used by the transaction
    recent_blockhash: String,
    /// Whether this transaction involves Pump.fun program
    is_pump_fun_transaction: bool,
    /// Type of Pump.fun instruction if applicable
    pump_fun_instruction_type: Option<PumpFunInstructionType>,
    /// Processing timestamp
    processed_at: DateTime<Utc>,
}

/// Represents the state of a bonding curve for a token
/// This struct mirrors the on-chain account structure for pump.fun bonding curves
#[derive(Debug, Clone, BorshDeserialize)]
struct BondingCurveState {
    discriminator: u64,
    /// Virtual token reserves (for pricing calculations)
    virtual_token_reserves: u64,
    /// Virtual SOL reserves (for pricing calculations)
    virtual_sol_reserves: u64,
    /// Real token reserves (actual tokens remaining)
    real_token_reserves: u64,
    /// Real SOL reserves (actual SOL collected)
    real_sol_reserves: u64,
    /// Total supply of the token
    token_total_supply: u64,
    /// Whether the curve has completed and migrated
    complete: bool,
    creator: Pubkey,
    /// When this state was last updated (not part of on-chain data)
    #[borsh(skip)]
    last_updated: DateTime<Utc>,
}

impl BondingCurveState {
    /// Calculate the graduation progress percentage (0-100)
    /// Based on: ((INITIAL_VIRTUAL_TOKEN_RESERVES - virtual_token_reserves) * 100) / TOTAL_SELLABLE_TOKENS
    pub fn calculate_graduation_progress(&self) -> f64 {
        if self.virtual_token_reserves >= INITIAL_VIRTUAL_TOKEN_RESERVES {
            return 0.0;
        }
        
        let tokens_sold = INITIAL_VIRTUAL_TOKEN_RESERVES - self.virtual_token_reserves;
        (tokens_sold as f64 * 100.0) / TOTAL_SELLABLE_TOKENS as f64
    }
    
    /// Calculate current market cap in SOL using bonding curve formula
    /// Using virtual reserves for accurate pricing: virtual_sol_reserves / virtual_token_reserves
    pub fn calculate_market_cap_sol(&self) -> f64 {
        if self.virtual_token_reserves == 0 {
            return 0.0;
        }
        
        // Current price per token in SOL
        let price_per_token = self.virtual_sol_reserves as f64 / self.virtual_token_reserves as f64;
        
        // Market cap = price * circulating supply
        let circulating_supply = self.token_total_supply - self.real_token_reserves;
        (price_per_token * circulating_supply as f64) / 1_000_000_000.0 // Convert from lamports
    }
    
    /// Calculate SOL needed to reach graduation threshold
    pub fn sol_needed_for_graduation(&self) -> f64 {
        let current_sol = self.real_sol_reserves as f64 / 1_000_000_000.0; // Convert from lamports
        (GRADUATION_SOL_THRESHOLD - current_sol).max(0.0)
    }
    
    /// Check if token is close to graduation (above any threshold)
    pub fn is_close_to_graduation(&self) -> Option<f64> {
        let progress = self.calculate_graduation_progress();
        GRADUATION_ALERT_THRESHOLDS.iter()
            .find(|&&threshold| progress >= threshold)
            .copied()
    }
}

/// Tracks graduation progress for tokens
#[derive(Debug)]
struct GraduationTracker {
    /// Map of mint addresses to their bonding curve states
    bonding_curves: DashMap<String, BondingCurveState>,
    /// Track which graduation thresholds have been alerted for each token
    alerted_thresholds: DashMap<String, Vec<f64>>,
}

impl GraduationTracker {
    /// Create a new graduation tracker
    pub fn new() -> Self {
        info!("Initializing graduation tracker for about-to-graduate tokens");
        Self {
            bonding_curves: DashMap::new(),
            alerted_thresholds: DashMap::new(),
        }
    }
    
    /// Update bonding curve state for a token
    pub fn update_bonding_curve(&self, mint: String, state: BondingCurveState) {
        debug!("Updating bonding curve state for token {}", mint);
        
        let _progress = state.calculate_graduation_progress();
        let _market_cap = state.calculate_market_cap_sol();
        let _sol_needed = state.sol_needed_for_graduation();
        
        // Check if we should alert for new graduation thresholds
        if let Some(current_threshold) = state.is_close_to_graduation() {
            let mut should_alert = false;
            
            self.alerted_thresholds.entry(mint.clone()).and_modify(|thresholds| {
                if !thresholds.contains(&current_threshold) {
                    thresholds.push(current_threshold);
                    should_alert = true;
                }
            }).or_insert_with(|| {
                should_alert = true;
                vec![current_threshold]
            });
            
            if should_alert {
                self.log_graduation_alert(&mint, &state, current_threshold);
            }
        }
        
        // Update the bonding curve state
        self.bonding_curves.insert(mint, state);
    }
    
    /// Log graduation alert for a token reaching a new threshold
    fn log_graduation_alert(&self, mint: &str, state: &BondingCurveState, threshold: f64) {
        let progress = state.calculate_graduation_progress();
        let market_cap = state.calculate_market_cap_sol();
        let sol_needed = state.sol_needed_for_graduation();
        let current_sol = state.real_sol_reserves as f64 / 1_000_000_000.0;
        
        warn!("🎯 GRADUATION ALERT ({}%): {}", threshold, mint);
        info!("  Graduation progress: {:.2}%", progress);
        info!("  Current market cap: {:.4} SOL", market_cap);
        info!("  SOL collected: {:.4} / {:.1}", current_sol, GRADUATION_SOL_THRESHOLD);
        info!("  SOL needed: {:.4}", sol_needed);
        info!("  Virtual token reserves: {}", state.virtual_token_reserves);
        info!("  Real token reserves: {}", state.real_token_reserves);
        
        if progress >= 95.0 {
            warn!("  🚨 CRITICAL: Token is very close to graduation!");
        }
    }
    
    /// Get all tokens that are close to graduation (above 50% threshold)
    pub fn get_tokens_close_to_graduation(&self) -> Vec<(String, f64, f64)> {
        self.bonding_curves.iter()
            .filter_map(|entry| {
                let (mint, state) = (entry.key(), entry.value());
                let progress = state.calculate_graduation_progress();
                if progress >= 50.0 {
                    Some((mint.clone(), progress, state.calculate_market_cap_sol()))
                } else {
                    None
                }
            })
            .collect()
    }
    
    /// Get graduation statistics
    pub fn get_graduation_stats(&self) -> (usize, usize, usize) {
        let total_tracked = self.bonding_curves.len();
        let close_to_graduation = self.get_tokens_close_to_graduation().len();
        let completed = self.bonding_curves.iter()
            .filter(|entry| entry.value().complete)
            .count();
            
        (total_tracked, close_to_graduation, completed)
    }
}

/// Main scanner struct that manages the blockchain scanning process
pub struct SolanaBlockScanner {
    /// RPC client for connecting to Solana network (wrapped in Arc for sharing)
    rpc_client: Arc<RpcClient>,
    /// Cache to track processed blocks and avoid duplicates
    processed_blocks: DashMap<u64, ProcessedBlock>,
    /// Current slot being processed
    current_slot: u64,
    /// Configuration for polling interval
    polling_interval: Duration,
    /// Maximum blocks to process (0 for unlimited)
    max_blocks: u64,
    /// Token tracker for logging
    token_tracker: TokenTracker,
    /// Graduation tracker for about-to-graduate tokens
    graduation_tracker: GraduationTracker,
}

impl SolanaBlockScanner {
    /// Create a new instance of the block scanner
    ///
    /// # Arguments
    /// * `rpc_url` - The Solana RPC endpoint URL
    /// * `start_slot` - Starting slot number (0 for latest)
    /// * `polling_interval` - How often to poll for new blocks
    /// * `max_blocks` - Maximum number of blocks to process
    ///
    /// # Returns
    /// * `Result<Self>` - New scanner instance or error
    pub async fn new(
        rpc_url: String,
        start_slot: u64,
        polling_interval: Duration,
        max_blocks: u64,
    ) -> Result<Self> {
        info!("🚀 Initializing Solana Transaction Scanner");
        info!("📡 Connecting to RPC endpoint: {}", rpc_url);

        // Initialize RPC client with confirmed commitment for reliability
        let rpc_client = Arc::new(RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed()));

        // Test connection by getting current slot (run in blocking task to avoid blocking async runtime)
        let latest_slot = tokio::task::spawn_blocking({
            let client = Arc::clone(&rpc_client);
            move || client.get_slot()
        })
        .await
        .context("Task join error")?
        .context("Failed to connect to Solana RPC endpoint")?;

        info!("✅ Successfully connected to Solana network");
        info!("🔢 Latest confirmed slot: {}", latest_slot);

        // Determine starting slot - use latest if 0 provided
        let current_slot = if start_slot == 0 {
            latest_slot
        } else {
            start_slot
        };

        info!("🎯 Starting scan from slot: {}", current_slot);

        let token_tracker = TokenTracker::new();

        // Initialize graduation tracker
        let graduation_tracker = GraduationTracker::new();

        Ok(Self {
            rpc_client,
            processed_blocks: DashMap::new(),
            current_slot,
            polling_interval,
            max_blocks,
            token_tracker,
            graduation_tracker,
        })
    }

    /// Start the main scanning loop
    ///
    /// This method runs indefinitely, polling for new blocks and processing their transactions
    ///
    /// # Returns
    /// * `Result<()>` - Success or error if scanning fails
    pub async fn start_scanning(&mut self) -> Result<()> {
        info!("Starting blockchain transaction scanning loop");
        
        if self.max_blocks > 0 {
            info!("Will process maximum {} blocks", self.max_blocks);
        } else {
            info!("Will process blocks indefinitely");
        }

        // Create interval timer for polling
        let mut poll_timer = interval(self.polling_interval);
        
        // Create interval timer for cleaning up old tokens (every 5 minutes)
        let mut cleanup_timer = interval(Duration::from_secs(300));

        loop {
            tokio::select! {
                // Handle polling for new blocks
                _ = poll_timer.tick() => {
                    // Check if we've reached the maximum block limit
                    if self.max_blocks > 0 && self.current_slot >= self.max_blocks {
                        info!("Reached maximum block limit ({}), stopping scanner", self.max_blocks);
                        break;
                    }

                    // Process the next block
                    match self.process_next_block().await {
                        Ok(processed) => {
                            if processed {
                                self.current_slot += 1;
                                debug!("Total blocks processed: {}", self.current_slot);
                            }
                        }
                        Err(e) => {
                            error!("Error processing block at slot {}: {}", self.current_slot, e);
                            sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
                
                // Handle cleanup of old tokens
                _ = cleanup_timer.tick() => {
                    // Use our comprehensive periodic stats function
                    self.log_periodic_stats().await;
                }
                
                // Handle graceful shutdown on Ctrl+C
                _ = tokio::signal::ctrl_c() => {
                    info!("Received interrupt signal, shutting down gracefully...");
                    break;
                }
            }
        }

        info!("Scanning completed. Blocks processed: {}", self.current_slot);
        
        // Log final graduation stats
        self.log_periodic_stats().await;
        
        Ok(())
    }

    /// Process the next block in sequence
    ///
    /// # Returns
    /// * `Result<bool>` - True if block was processed, False if block not yet available
    async fn process_next_block(&mut self) -> Result<bool> {
        // Check if block has already been processed
        if self.processed_blocks.contains_key(&self.current_slot) {
            debug!("⏭️  Block at slot {} already processed, skipping", self.current_slot);
            self.current_slot += 1;
            return Ok(false);
        }

        // Attempt to get block data from RPC (use spawn_blocking for sync RPC call)
        debug!("🔍 Fetching block data for slot: {}", self.current_slot);
        
        let block_result = tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            let slot = self.current_slot;
            move || {
                client.get_block_with_config(
                    slot,
                    solana_client::rpc_config::RpcBlockConfig {
                        encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                        transaction_details: Some(
                            solana_transaction_status::TransactionDetails::Full,
                        ),
                        rewards: Some(false),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                    },
                )
            }
        })
        .await
        .context("Task join error")?;
        
        match block_result {
            Ok(block) => {
                // Successfully retrieved block, process it
                let processed_block = self.create_processed_block(&block, self.current_slot)?;
                
                // Process all transactions in the block
                self.process_block_transactions(&block, &processed_block).await?;
                
                // Cache the processed block
                self.processed_blocks.insert(self.current_slot, processed_block);
                
                // Move to next slot
                self.current_slot += 1;
                Ok(true)
            }
            Err(e) => {
                // Block might not be available yet or other error occurred
                debug!("⏳ Block at slot {} not available yet: {}", self.current_slot, e);
                
                // Check if we need to skip ahead (block might be missing)
                let latest_slot = tokio::task::spawn_blocking({
                    let client = Arc::clone(&self.rpc_client);
                    move || client.get_slot()
                })
                .await
                .context("Task join error")?
                .context("Failed to get latest slot")?;
                
                if self.current_slot < latest_slot.saturating_sub(100) {
                    warn!("⚠️  Slot {} appears to be missing, skipping to next", self.current_slot);
                    self.current_slot += 1;
                }
                
                Ok(false)
            }
        }
    }

    /// Process all transactions in a block
    ///
    /// # Arguments
    /// * `block` - The block data from RPC
    /// * `processed_block` - Metadata about the processed block
    ///
    /// # Returns
    /// * `Result<()>` - Success or error if transaction processing fails
    async fn process_block_transactions(
        &mut self,
        block: &solana_transaction_status::UiConfirmedBlock,
        processed_block: &ProcessedBlock,
    ) -> Result<()> {
        // Get transactions from the block
        let transactions = match &block.transactions {
            Some(txs) => txs,
            None => {
                debug!("No transactions in block {}", processed_block.slot);
                return Ok(());
            }
        };

        // Process each transaction and filter for Pump.fun transactions
        for (tx_index, transaction) in transactions.iter().enumerate() {
            // Check if this transaction involves Pump.fun program
            if self.is_pump_fun_transaction(transaction) {
                // Process CREATE, BUY, and MIGRATE transactions
                if self.has_instruction_type(transaction, &CREATE_INSTRUCTION_DISCRIMINATOR) ||
                   self.has_instruction_type(transaction, &BUY_INSTRUCTION_DISCRIMINATOR) ||
                   self.has_instruction_type(transaction, &MIGRATE_INSTRUCTION_DISCRIMINATOR) {
                    
                    // Check if the transaction was successful
                    let is_successful = transaction.meta.as_ref()
                        .map(|meta| meta.err.is_none())
                        .unwrap_or(false);
                    
                    // Skip failed transactions
                    if !is_successful {
                        debug!("Skipping failed Pump.fun transaction");
                        continue;
                    }
                    
                    // Process and log successful Pump.fun transactions
                    match self.process_single_transaction(transaction, processed_block, tx_index).await {
                        Ok(processed_tx) => {
                            // Handle CREATE and BUY transactions
                            if processed_tx.pump_fun_instruction_type == Some(PumpFunInstructionType::Create) {
                                let tx_type = processed_tx.pump_fun_instruction_type.as_ref().unwrap();
                                info!("Found Pump.fun {:?} transaction: {}", tx_type, processed_tx.signature);

                                // Extract mint address for tracking
                                if let Some(mint) = self.extract_mint_address_from_transaction(transaction, tx_type) {
                                    info!("  Token Mint: {}", mint);
                                    
                                    // Extract SOL amount spent (only for buy/create transactions)
                                    let sol_amount = self.extract_sol_amount_from_buy(transaction);
                                    if let Some(sol_amt) = sol_amount {
                                        info!("  SOL Spent: {:.4} SOL", sol_amt);
                                        
                                        // Record this transaction for hot token tracking
                                        self.token_tracker.record_transaction(
                                            mint.clone(),
                                            processed_tx.signature.clone(),
                                            sol_amt,
                                            processed_tx.processed_at,
                                            tx_type.clone()
                                        );
                                    }

                                    // Log transaction details
                                    info!("📊 {} Transaction: {} ({})",
                                        format!("{:?}", tx_type),
                                        mint,
                                        processed_tx.signature,
                                    );

                                    if let Some(sol_amt) = sol_amount {
                                        if sol_amt > 0.0 {
                                            info!("  💰 SOL amount: {:.4}", sol_amt);
                                        }
                                    }
                                }
                            
                                info!("  Slot: {} Block: {}", processed_tx.slot, processed_tx.block_hash);
                            } else if processed_tx.pump_fun_instruction_type == Some(PumpFunInstructionType::Buy) ||
                                processed_tx.pump_fun_instruction_type == Some(PumpFunInstructionType::Sell) {
                                    let tx_type = processed_tx.pump_fun_instruction_type.as_ref().unwrap();

                                    if let Some(mint) = self.extract_mint_address_from_transaction(transaction, tx_type) {                                        
                                        // Extract and update bonding curve state for buy transactions
                                        if let Some(bonding_curve_state) = self.extract_bonding_curve_from_transaction(transaction) {
                                            self.graduation_tracker.update_bonding_curve(
                                                mint.clone(),
                                                bonding_curve_state,
                                            );
                                        }
                                    }
                            } else if processed_tx.pump_fun_instruction_type == Some(PumpFunInstructionType::Migrate) {
                                info!("Found Pump.fun MIGRATE transaction: {}", processed_tx.signature);

                                // Extract mint address for tracking
                                if let Some(mint) = self.extract_mint_address_from_transaction(transaction, &PumpFunInstructionType::Migrate) {
                                    info!("  Token Mint: {}", mint);
                                    
                                    // Record this migration (no SOL amount for migrations)
                                    self.token_tracker.record_transaction(
                                        mint,
                                        processed_tx.signature.clone(),
                                        0.0, // No SOL spent in migrations
                                        processed_tx.processed_at,
                                        PumpFunInstructionType::Migrate
                                    );
                                }
                            
                                info!("  Slot: {} Block: {}", processed_tx.slot, processed_tx.block_hash);
                            }
                        }
                        Err(e) => {
                            debug!("Failed to process Pump.fun transaction in block {}: {}", 
                                  processed_block.slot, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a transaction involves the Pump.fun program
    ///
    /// # Arguments
    /// * `transaction` - The transaction to check
    ///
    /// # Returns
    /// * `bool` - True if the transaction involves Pump.fun program
    fn is_pump_fun_transaction(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
    ) -> bool {
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // Check if Pump.fun program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == MOONIT_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Pump.fun program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == MOONIT_PROGRAM_ID)
                    }
                }
            }
            _ => {
                // For non-JSON formats, we can't easily check, so return false
                false
            }
        }
    }

    /// Check if a transaction contains a specific Pump.fun instruction type
    ///
    /// # Arguments
    /// * `transaction` - The transaction to check
    /// * `discriminator` - The 8-byte instruction discriminator to look for
    ///
    /// # Returns
    /// * `bool` - True if the transaction contains the specified instruction type
    fn has_instruction_type(
        &self, 
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        discriminator: &[u8; 8],
    ) -> bool {
        // Process instructions based on transaction type
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, we need to check each instruction 
                        // differently since they have a different structure
                        for instruction in &parsed_message.instructions {
                            // For parsed instructions, we need to get program_id and data
                            // differently based on whether it's a parsed instruction or not
                            let (_program_id, data) = match instruction {
                                solana_transaction_status::UiInstruction::Parsed(_parsed_instruction) => {
                                    // For parsed instructions, we don't have direct access to raw data
                                    // Let's skip them for now and rely on the compiled instructions
                                    continue;
                                },
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Get program ID from account keys and program_id_index
                                    let program_idx = compiled.program_id_index as usize;
                                    if program_idx >= parsed_message.account_keys.len() {
                                        continue;
                                    }

                                    let program_id = &parsed_message.account_keys[program_idx].pubkey;
                                    if program_id != MOONIT_PROGRAM_ID {
                                        continue;
                                    }

                                    (program_id, &compiled.data)
                                }
                            };

                            // Now check the instruction data for the discriminator
                            // Decode instruction data from base58
                            if let Ok(decoded_data) = bs58::decode(data).into_vec() {
                                // Check if data starts with our target discriminator
                                if decoded_data.len() >= 8 && decoded_data[0..8] == discriminator[..] {
                                    return true;
                                }
                            }
                        }
                        false
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Handle raw instructions
                        for instruction in &raw_message.instructions {
                            // For raw instructions, we need to map program_id index to the actual ID
                            let program_id = if instruction.program_id_index < raw_message.account_keys.len() as u8 {
                                &raw_message.account_keys[instruction.program_id_index as usize]
                            } else {
                                continue;
                            };
                            
                            if program_id == MOONIT_PROGRAM_ID {
                                // Decode instruction data from base58
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    // Check if data starts with our target discriminator
                                    if data.len() >= 8 && data[0..8] == discriminator[..] {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                }
            }
            _ => return false,
        }
    }

    /// Determine the Pump.fun instruction type for a transaction
    ///
    /// # Arguments
    /// * `transaction` - The transaction to analyze
    ///
    /// # Returns
    /// * `Option<PumpFunInstructionType>` - The instruction type if it's a Pump.fun transaction
    fn determine_pump_fun_instruction_type(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
    ) -> Option<PumpFunInstructionType> {
        if !self.is_pump_fun_transaction(transaction) {
            return None;
        }

        if self.has_instruction_type(transaction, &CREATE_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Create)
        } else if self.has_instruction_type(transaction, &SELL_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Sell)
        } else if self.has_instruction_type(transaction, &BUY_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Buy)
        } else if self.has_instruction_type(transaction, &MIGRATE_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Migrate)
        } else {
            Some(PumpFunInstructionType::Other)
        }
    }

    /// Process a single transaction and extract relevant information
    ///
    /// # Arguments
    /// * `transaction` - The transaction data from the block
    /// * `processed_block` - Metadata about the block containing this transaction
    /// * `tx_index` - Index of the transaction within the block
    ///
    /// # Returns
    /// * `Result<ProcessedTransaction>` - Processed transaction data or error
    async fn process_single_transaction(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        processed_block: &ProcessedBlock,
        tx_index: usize,
    ) -> Result<ProcessedTransaction> {
        // Extract transaction signature (first signature is the transaction signature)
        let signature = match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                ui_transaction.signatures.first()
                    .ok_or_else(|| anyhow::anyhow!("Transaction missing signature"))?
                    .clone()
            }
            solana_transaction_status::EncodedTransaction::LegacyBinary(_) => {
                return Err(anyhow::anyhow!("Legacy binary format not supported"));
            }
            solana_transaction_status::EncodedTransaction::Binary(_, _) => {
                return Err(anyhow::anyhow!("Binary format not supported"));
            }
            solana_transaction_status::EncodedTransaction::Accounts(_) => {
                return Err(anyhow::anyhow!("Accounts format not supported"));
            }
        };

        // Determine if transaction was successful
        let is_successful = transaction.meta.as_ref()
            .map(|meta| meta.err.is_none())
            .unwrap_or(false);

        // Extract error message if transaction failed
        let error_message = transaction.meta.as_ref()
            .and_then(|meta| meta.err.as_ref())
            .map(|err| format!("{:?}", err));

        // Extract fee information
        let fee = transaction.meta.as_ref()
            .map(|meta| meta.fee)
            .unwrap_or(0);

        // Extract compute units consumed (handle OptionSerializer)
        let compute_units_consumed = transaction.meta.as_ref()
            .and_then(|meta| {
                match &meta.compute_units_consumed {
                    solana_transaction_status::option_serializer::OptionSerializer::Some(units) => Some(*units),
                    solana_transaction_status::option_serializer::OptionSerializer::None => None,
                    solana_transaction_status::option_serializer::OptionSerializer::Skip => None,
                }
            });

        // Extract account keys and recent blockhash from transaction message
        let (account_keys, recent_blockhash, instruction_count) = match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        let account_keys = parsed_message.account_keys.iter()
                            .map(|key| key.pubkey.clone())
                            .collect();
                        let recent_blockhash = parsed_message.recent_blockhash.clone();
                        let instruction_count = parsed_message.instructions.len();
                        (account_keys, recent_blockhash, instruction_count)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        let account_keys = raw_message.account_keys.clone();
                        let recent_blockhash = raw_message.recent_blockhash.clone();
                        let instruction_count = raw_message.instructions.len();
                        (account_keys, recent_blockhash, instruction_count)
                    }
                }
            }
            _ => {
                // For non-JSON formats, provide defaults
                (vec![], format!("unknown_{}", tx_index), 0)
            }
        };

        // Check if this is a Pump.fun transaction and determine transaction type
        let is_pump_fun_transaction = account_keys.iter().any(|key| key == MOONIT_PROGRAM_ID);
        let pump_fun_instruction_type = if is_pump_fun_transaction {
            self.determine_pump_fun_instruction_type(transaction)
        } else {
            None
        };

        Ok(ProcessedTransaction {
            signature,
            slot: processed_block.slot,
            block_hash: processed_block.blockhash.clone(),
            is_successful,
            error_message,
            instruction_count,
            compute_units_consumed,
            fee,
            account_keys,
            recent_blockhash,
            is_pump_fun_transaction,
            pump_fun_instruction_type,
            processed_at: Utc::now(),
        })
    }

    /// Create a ProcessedBlock from RPC block data
    ///
    /// # Arguments
    /// * `block` - Block data from RPC
    /// * `slot` - Slot number
    ///
    /// # Returns
    /// * `Result<ProcessedBlock>` - Processed block metadata
    fn create_processed_block(
        &self,
        block: &solana_transaction_status::UiConfirmedBlock,
        slot: u64,
    ) -> Result<ProcessedBlock> {
        // Extract block timestamp if available
        let timestamp = block.block_time.map(|ts| {
            DateTime::from_timestamp(ts, 0)
                .unwrap_or_else(|| Utc::now())
        });

        // Get transaction count
        let transaction_count = block.transactions.as_ref()
            .map(|txs| txs.len())
            .unwrap_or(0);

        // Create processed block record
        let processed_block = ProcessedBlock {
            slot,
            blockhash: block.blockhash.clone(),
            parent_slot: block.parent_slot,
            timestamp,
            transaction_count,
            processed_at: Utc::now(),
        };

        debug!("✅ Created processed block record for slot {}", slot);
        Ok(processed_block)
    }

    /// Extract the mint address from a transaction based on instruction type
    ///
    /// # Arguments
    /// * `transaction` - The transaction containing the instruction
    /// * `instruction_type` - The type of instruction to extract from
    ///
    /// # Returns
    /// * `Option<String>` - The mint address if found
    fn extract_mint_address_from_transaction(
        &self, 
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        instruction_type: &PumpFunInstructionType
    ) -> Option<String> {
        // Get the appropriate discriminator for the instruction type
        let discriminator = match instruction_type {
            PumpFunInstructionType::Buy => &BUY_INSTRUCTION_DISCRIMINATOR,
            PumpFunInstructionType::Sell => &SELL_INSTRUCTION_DISCRIMINATOR,
            PumpFunInstructionType::Create => &CREATE_INSTRUCTION_DISCRIMINATOR,
            PumpFunInstructionType::Migrate => &MIGRATE_INSTRUCTION_DISCRIMINATOR,
            _ => return None,
        };

        // Find the instruction with the matching discriminator
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, check each instruction
                        for instruction in &parsed_message.instructions {
                            match instruction {
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Check if this matches our target instruction type
                                    if let Ok(data) = bs58::decode(&compiled.data).into_vec() {
                                        if data.len() >= 8 && data[0..8] == discriminator[..] {
                                            // For different instruction types, the mint account is at different positions
                                            let mint_account_index = match instruction_type {
                                                PumpFunInstructionType::Buy | PumpFunInstructionType::Sell | PumpFunInstructionType::Create => {
                                                    // For buy/sell/create, mint is typically the 3rd account (index 2)
                                                    if compiled.accounts.len() > 2 { Some(2) } else { None }
                                                }
                                                PumpFunInstructionType::Migrate => {
                                                    // For migrate, mint is typically the 2nd account (index 1)
                                                    if compiled.accounts.len() > 1 { Some(1) } else { None }
                                                }
                                                _ => None,
                                            };
                                            
                                            if let Some(account_idx) = mint_account_index {
                                                let mint_idx = compiled.accounts[account_idx] as usize;
                                                if mint_idx < parsed_message.account_keys.len() {
                                                    return Some(parsed_message.account_keys[mint_idx].pubkey.clone());
                                                }
                                            }
                                        }
                                    }
                                },
                                _ => continue,
                            }
                        }
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // For raw messages, check each instruction
                        for instruction in &raw_message.instructions {
                            // Check if this matches our target instruction type
                            if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                if data.len() >= 8 && data[0..8] == discriminator[..] {
                                    // For different instruction types, the mint account is at different positions
                                    let mint_account_index = match instruction_type {
                                        PumpFunInstructionType::Buy | PumpFunInstructionType::Sell | PumpFunInstructionType::Create => {
                                            // For buy/create, mint is typically the 3rd account (index 2)
                                            if instruction.accounts.len() > 2 { Some(2) } else { None }
                                        }
                                        PumpFunInstructionType::Migrate => {
                                            // For migrate, mint is typically the 2nd account (index 1)
                                            if instruction.accounts.len() > 1 { Some(1) } else { None }
                                        }
                                        _ => None,
                                    };
                                    
                                    if let Some(account_idx) = mint_account_index {
                                        let mint_idx = instruction.accounts[account_idx] as usize;
                                        if mint_idx < raw_message.account_keys.len() {
                                            return Some(raw_message.account_keys[mint_idx].clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            _ => return None,
        }
        
        None
    }

    /// Extract the SOL amount spent in a buy transaction
    ///
    /// # Arguments
    /// * `transaction` - The transaction containing the buy instruction
    ///
    /// # Returns
    /// * `Option<f64>` - The SOL amount spent if it can be determined
    fn extract_sol_amount_from_buy(&self, transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> Option<f64> {
        // We need transaction metadata to get balances
        let meta = transaction.meta.as_ref()?;
        
        // Try to find buyer account index
        let mut buyer_index: Option<usize> = None;
        
        // Extract buyer account index from transaction
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, check each instruction
                        for instruction in &parsed_message.instructions {
                            match instruction {
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    if let Ok(data) = bs58::decode(&compiled.data).into_vec() {
                                        if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                            // Buyer is typically the first account in the accounts list
                                            if !compiled.accounts.is_empty() {
                                                buyer_index = Some(compiled.accounts[0] as usize);
                                                break;
                                            }
                                        }
                                    }
                                },
                                _ => continue,
                            }
                        }
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // For raw messages, check each instruction
                        for instruction in &raw_message.instructions {
                            if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                    // Buyer is typically the first account in the accounts list
                                    if !instruction.accounts.is_empty() {
                                        buyer_index = Some(instruction.accounts[0] as usize);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            },
            _ => {}
        };
        
        // If we found a buyer index, calculate SOL spent
        if let Some(idx) = buyer_index {
            if idx < meta.pre_balances.len() && idx < meta.post_balances.len() {
                let pre_balance = meta.pre_balances[idx];
                let post_balance = meta.post_balances[idx];
                
                // Calculate difference and convert from lamports to SOL
                if pre_balance > post_balance {
                    let lamports_spent = pre_balance - post_balance;
                    let sol_spent = lamports_spent as f64 / 1_000_000_000.0; // Convert lamports to SOL
                    return Some(sol_spent);
                }
            }
        }
        
        // Fallback to transaction fee if we couldn't determine the spent amount
        Some(meta.fee as f64 / 1_000_000_000.0)
    }

    /// Extract bonding curve data from a transaction by fetching the account state
    ///
    /// # Arguments
    /// * `transaction` - The transaction containing pump.fun instructions
    ///
    /// # Returns
    /// * `Option<BondingCurveState>` - The bonding curve state if successfully extracted
    fn extract_bonding_curve_from_transaction(&self, transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> Option<BondingCurveState> {
        // Extract the bonding curve account address from the transaction
        let bonding_curve_address = self.extract_bonding_curve_address_from_transaction(transaction)?;
        
        // Convert string address to Pubkey
        let bonding_curve_pubkey = match bonding_curve_address.parse::<Pubkey>() {
            Ok(pubkey) => pubkey,
            Err(e) => {
                debug!("Failed to parse bonding curve address {}: {}", bonding_curve_address, e);
                return None;
            }
        };
        
        // Fetch account data from the blockchain
        match self.rpc_client.get_account(&bonding_curve_pubkey) {
            Ok(account_info) => {
                // Deserialize the account data using Borsh
                let mut data_slice = account_info.data.as_slice();
                match BondingCurveState::deserialize(&mut data_slice) {
                    Ok(mut bonding_curve_state) => {
                        // Set the last_updated timestamp
                        bonding_curve_state.last_updated = Utc::now();
                        debug!("Successfully extracted bonding curve state for {}: real_sol_reserves={} lamports ({:.4} SOL), real_token_reserves={}, complete={}", 
                              bonding_curve_address, 
                              bonding_curve_state.real_sol_reserves,
                              bonding_curve_state.real_sol_reserves as f64 / 1_000_000_000.0,
                              bonding_curve_state.real_token_reserves,
                              bonding_curve_state.complete,
                              );
                        
                        Some(bonding_curve_state)
                    },
                    Err(e) => {
                        info!("Failed to deserialize bonding curve account data for {}: {}", bonding_curve_address, e);
                        debug!("Account data length: {} bytes", account_info.data.len());
                        None
                    }
                }
            },
            Err(e) => {
                debug!("Failed to fetch bonding curve account {} from RPC: {}", bonding_curve_address, e);
                None
            }
        }
    }

    /// Extract bonding curve account address from a pump.fun transaction
    ///
    /// # Arguments  
    /// * `transaction` - The transaction to extract the address from
    ///
    /// # Returns
    /// * `Option<String>` - The bonding curve account address if found
    fn extract_bonding_curve_address_from_transaction(&self, transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> Option<String> {
        // Process transaction based on its encoding type
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For buy transactions, the bonding curve is typically at index 2 or 3
                        // We need to check the instruction to find the correct account index
                        for instruction in &parsed_message.instructions {
                            if let solana_transaction_status::UiInstruction::Compiled(compiled) = instruction {
                                let program_idx = compiled.program_id_index as usize;
                                if program_idx >= parsed_message.account_keys.len() {
                                    continue;
                                }

                                let program_id = &parsed_message.account_keys[program_idx].pubkey;
                                if program_id != MOONIT_PROGRAM_ID {
                                    continue;
                                }

                                // Check if this is a buy instruction
                                if let Ok(decoded_data) = bs58::decode(&compiled.data).into_vec() {
                                    if decoded_data.len() >= 8 && (decoded_data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || decoded_data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy/sell instructions, bonding curve is typically at account index 3
                                        if compiled.accounts.len() > 3 {
                                            let bonding_curve_index = compiled.accounts[3] as usize;
                                            if bonding_curve_index < parsed_message.account_keys.len() {
                                                return Some(parsed_message.account_keys[bonding_curve_index].pubkey.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        None
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Handle raw instructions
                        for instruction in &raw_message.instructions {
                            let program_id = if instruction.program_id_index < raw_message.account_keys.len() as u8 {
                                &raw_message.account_keys[instruction.program_id_index as usize]
                            } else {
                                continue;
                            };
                            
                            if program_id == MOONIT_PROGRAM_ID {
                                // Check if this is a buy instruction
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                        // For buy instructions, bonding curve is typically at account index 3
                                        if instruction.accounts.len() > 3 {
                                            let bonding_curve_index = instruction.accounts[3] as usize;
                                            if bonding_curve_index < raw_message.account_keys.len() {
                                                return Some(raw_message.account_keys[bonding_curve_index].clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// Log periodic scanner statistics and hot token information
    async fn log_periodic_stats(&self) {
        let (total_tracked, close_to_graduation, completed) = self.graduation_tracker.get_graduation_stats();
        let close_tokens = self.graduation_tracker.get_tokens_close_to_graduation();

        info!("📈 SCANNER STATISTICS:");
        info!("  Total tokens tracked: {}", total_tracked);
        info!("  Close to graduation (>50%): {}", close_to_graduation);
        info!("  Completed tokens: {}", completed);

        if !close_tokens.is_empty() {
            info!("🎯 Tokens close to graduation:");
            for (i, (mint, progress, market_cap)) in close_tokens.iter().take(5).enumerate() {
                info!("  {}. {} - {:.1}% progress, {:.4} SOL market cap", 
                      i + 1, mint, progress, market_cap);
            }
            if close_tokens.len() > 5 {
                info!("  ... and {} more", close_tokens.len() - 5);
            }
        }
    }
}

/// Main application entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Initialize logging system
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();

    // Parse command line arguments
    let args = Args::parse();

    // Determine polling interval in milliseconds
    let polling_interval_ms = if args.interval_ms > 0 {
        args.interval_ms
    } else {
        args.interval * 1000
    };

    info!("Starting Solana Pump.fun Transaction Scanner v{}", env!("CARGO_PKG_VERSION"));
    info!("This scanner specifically monitors Pump.fun transactions");
    info!("Configuration:");
    info!("  RPC URL: {}", args.rpc_url);
    info!("  Polling Interval: {}ms", polling_interval_ms);
    info!("  Start Slot: {}", if args.start_slot == 0 { "latest".to_string() } else { args.start_slot.to_string() });
    info!("  Max Blocks: {}", if args.max_blocks == 0 { "unlimited".to_string() } else { args.max_blocks.to_string() });

    // Create the scanner (original code uses the same RPC URL, polling interval, etc.)
    let mut scanner = SolanaBlockScanner::new(
        args.rpc_url,
        args.start_slot,
        Duration::from_millis(polling_interval_ms),
        args.max_blocks,
    )
    .await
    .context("Failed to initialize transaction scanner")?;
    
    scanner.token_tracker = TokenTracker::new();

    // Handle graceful shutdown on Ctrl+C
    tokio::select! {
        result = scanner.start_scanning() => {
            match result {
                Ok(()) => info!("Scanner completed successfully"),
                Err(e) => error!("Scanner failed: {}", e),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received interrupt signal, shutting down gracefully...");
            let (total_tracked, close_to_graduation, completed) = scanner.graduation_tracker.get_graduation_stats();
            info!("Final Statistics:");
            info!("  Total tokens tracked: {}", total_tracked);
            info!("  Close to graduation (>50%): {}", close_to_graduation);
            info!("  Completed tokens: {}", completed);
        }
    }

    info!("Solana Pump.fun Transaction Scanner shutdown complete");
    Ok(())
} 