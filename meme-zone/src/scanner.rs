use anyhow::{Context, Result};
use chrono::Utc;
use dashmap::DashMap;
use log::{debug, error, info};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{EncodedTransactionWithStatusMeta, UiTransactionEncoding};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;
use uuid::Uuid;

use crate::platform_trait::{BondingCurveStateTrait, InstructionType, TokenPlatformTrait};

/// Structure to track token statistics
#[derive(Debug, Clone)]
pub struct TokenState {
    pub mint_address: String,
    pub bonding_curve_address: String,
    pub first_seen: chrono::DateTime<Utc>,
    pub last_updated: chrono::DateTime<Utc>,
    pub graduation_progress: f64,
    pub market_cap: f64,
    pub needed_for_graduation: f64,
    pub is_graduated: bool,
    pub transactions_count: usize,
}

/// Main scanner that processes Solana blocks and transactions
pub struct SolanaBlockScanner {
    /// RPC client for connecting to Solana network
    rpc_client: Arc<RpcClient>,
    /// Platform strategy
    platform: Box<dyn TokenPlatformTrait>,
    /// Current slot being processed
    current_slot: u64,
    /// Maximum number of blocks to process
    max_blocks: u64,
    /// How long to wait between RPC requests
    polling_interval: Duration,
    /// Map of tokens we're tracking
    tokens: Arc<DashMap<String, TokenState>>,
    /// Blocks processed count
    blocks_processed: u64,
    /// Transactions processed count
    transactions_processed: u64,
    /// Platform transactions processed count
    platform_transactions_processed: u64,
    /// Scan ID for logging
    scan_id: String,
}

impl SolanaBlockScanner {
    /// Create a new scanner instance
    pub async fn new(
        rpc_url: String,
        start_slot: u64,
        polling_interval: Duration,
        max_blocks: u64,
        platform: Box<dyn TokenPlatformTrait>,
    ) -> Result<Self> {
        let rpc_client = Arc::new(RpcClient::new_with_commitment(
            rpc_url,
            CommitmentConfig::confirmed(),
        ));
        
        let scan_id = Uuid::new_v4().to_string();
        
        Ok(SolanaBlockScanner {
            rpc_client,
            platform,
            current_slot: start_slot,
            max_blocks,
            polling_interval,
            tokens: Arc::new(DashMap::new()),
            blocks_processed: 0,
            transactions_processed: 0,
            platform_transactions_processed: 0,
            scan_id,
        })
    }
    
    /// Start scanning blocks
    pub async fn start_scanning(&mut self) -> Result<()> {
        let platform_name = self.platform.name();
        info!("[{}] Starting {} scanner at slot {}", self.scan_id, platform_name, self.current_slot);
        
        let start_time = Instant::now();
        let mut next_stats_time = Instant::now() + Duration::from_secs(30);
        
        let mut blocks_processed = 0;
        
        while blocks_processed < self.max_blocks {
            match self.process_next_block().await {
                Ok(true) => {
                    blocks_processed += 1;
                    
                    // Log statistics periodically
                    if Instant::now() > next_stats_time {
                        self.log_statistics();
                        next_stats_time = Instant::now() + Duration::from_secs(30);
                    }
                }
                Ok(false) => {
                    // No new block yet, wait and try again
                    time::sleep(self.polling_interval).await;
                }
                Err(e) => {
                    error!("[{}] Error processing block: {}", self.scan_id, e);
                    time::sleep(self.polling_interval).await;
                }
            }
            
            // Check for graduation events
            self.check_graduation_status();
        }
        
        let duration = start_time.elapsed();
        info!(
            "[{}] Scanning complete. Processed {} blocks in {:?}",
            self.scan_id, blocks_processed, duration
        );
        
        Ok(())
    }
    
    /// Process the next block
    async fn process_next_block(&mut self) -> Result<bool> {
        // Get the current slot from the RPC
        let latest_slot = self
            .rpc_client
            .get_slot()
            .with_context(|| format!("Failed to get latest slot"))?;
        
        // If we're at the latest slot, there's no new block to process
        if self.current_slot > latest_slot {
            return Ok(false);
        }
        
        // Get the block data
        info!("[{}] Processing slot {}", self.scan_id, self.current_slot);
        
        let block_config = RpcBlockConfig {
            encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
            transaction_details: Some(solana_transaction_status::TransactionDetails::Full),
            rewards: Some(false),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };
        
        let block = self
            .rpc_client
            .get_block_with_config(self.current_slot, block_config)
            .with_context(|| format!("Failed to get block at slot {}", self.current_slot))?;
        
        // Process the block
        if let Some(transactions) = block.transactions {
            for transaction in transactions {
                self.process_transaction(&transaction).await?;
                self.transactions_processed += 1;
            }
        }
        
        // Move to the next slot
        self.current_slot += 1;
        self.blocks_processed += 1;
        
        Ok(true)
    }
    
    /// Process a single transaction
    async fn process_transaction(&mut self, transaction: &EncodedTransactionWithStatusMeta) -> Result<()> {
        // Skip if not related to our platform
        if !self.platform.is_platform_transaction(transaction) {
            return Ok(());
        }
        
        self.platform_transactions_processed += 1;
        
        // Determine instruction type
        let instruction_type = match self.platform.determine_instruction_type(transaction) {
            Some(t) => t,
            None => return Ok(()),
        };
        
        // Extract mint address
        let mint_address = match self.platform.extract_mint_address(transaction, &instruction_type) {
            Some(address) => address,
            None => {
                info!("[{}] Could not extract mint address", self.scan_id);
                return Ok(());
            }
        };
        
        // Extract bonding curve address for fetching state
        let bonding_curve_address = match self.platform.extract_bonding_curve_address(transaction) {
            Some(address) => address,
            None => {
                info!("[{}] Could not extract bonding curve address", self.scan_id);
                return Ok(());
            }
        };
        
        // Handle the transaction based on instruction type
        match instruction_type {
            InstructionType::Create => {
                self.handle_create_instruction(&mint_address, &bonding_curve_address).await?;
            }
            InstructionType::Buy => {
                if let Some(sol_amount) = self.platform.extract_sol_amount(transaction) {
                    self.handle_buy_instruction(&mint_address, &bonding_curve_address, sol_amount).await?;
                }
            }
            InstructionType::Sell => {
                self.handle_sell_instruction(&mint_address, &bonding_curve_address).await?;
            }
            InstructionType::Migrate => {
                self.handle_migrate_instruction(&mint_address, &bonding_curve_address).await?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Handle token creation instruction
    async fn handle_create_instruction(&self, mint_address: &str, bonding_curve_address: &str) -> Result<()> {
        info!(
            "[{}] New token created: {} (bonding curve: {})",
            self.scan_id, mint_address, bonding_curve_address
        );
        
        // Get initial bonding curve state
        if let Ok(state) = self.fetch_bonding_curve_state(bonding_curve_address).await {
            let now = Utc::now();
            
            // Create token state
            let token_state = TokenState {
                mint_address: mint_address.to_string(),
                bonding_curve_address: bonding_curve_address.to_string(),
                first_seen: now,
                last_updated: now,
                graduation_progress: state.calculate_graduation_progress(),
                market_cap: state.calculate_market_cap(),
                needed_for_graduation: state.needed_for_graduation(),
                is_graduated: false,
                transactions_count: 1,
            };
            
            // Store token state
            self.tokens.insert(mint_address.to_string(), token_state);
        }
        
        Ok(())
    }
    
    /// Handle buy instruction
    async fn handle_buy_instruction(&self, mint_address: &str, bonding_curve_address: &str, sol_amount: f64) -> Result<()> {
        info!(
            "[{}] Buy transaction for {} ({} SOL)",
            self.scan_id, mint_address, sol_amount
        );
        
        self.update_token_state(mint_address, bonding_curve_address).await?;
        
        Ok(())
    }
    
    /// Handle sell instruction
    async fn handle_sell_instruction(&self, mint_address: &str, bonding_curve_address: &str) -> Result<()> {
        info!("[{}] Sell transaction for {}", self.scan_id, mint_address);
        
        self.update_token_state(mint_address, bonding_curve_address).await?;
        
        Ok(())
    }
    
    /// Handle migrate instruction
    async fn handle_migrate_instruction(&self, mint_address: &str, bonding_curve_address: &str) -> Result<()> {
        info!("[{}] Migration for {}", self.scan_id, mint_address);
        
        self.update_token_state(mint_address, bonding_curve_address).await?;
        
        Ok(())
    }
    
    /// Update token state after a transaction
    async fn update_token_state(&self, mint_address: &str, bonding_curve_address: &str) -> Result<()> {
        // Fetch current bonding curve state
        if let Ok(state) = self.fetch_bonding_curve_state(bonding_curve_address).await {
            let now = Utc::now();
            
            // Update or create token state
            if let Some(mut token_entry) = self.tokens.get_mut(mint_address) {
                let token = &mut token_entry.value_mut();
                token.last_updated = now;
                token.graduation_progress = state.calculate_graduation_progress();
                token.market_cap = state.calculate_market_cap();
                token.needed_for_graduation = state.needed_for_graduation();
                token.transactions_count += 1;
                
                if state.is_complete() && !token.is_graduated {
                    token.is_graduated = true;
                    info!(
                        "[{}] Token {} has graduated! Market cap: {} SOL",
                        self.scan_id, mint_address, token.market_cap
                    );
                }
            } else {
                // If this is the first time we're seeing this token, create a new entry
                let token_state = TokenState {
                    mint_address: mint_address.to_string(),
                    bonding_curve_address: bonding_curve_address.to_string(),
                    first_seen: now,
                    last_updated: now,
                    graduation_progress: state.calculate_graduation_progress(),
                    market_cap: state.calculate_market_cap(),
                    needed_for_graduation: state.needed_for_graduation(),
                    is_graduated: state.is_complete(),
                    transactions_count: 1,
                };
                
                self.tokens.insert(mint_address.to_string(), token_state);
            }
        }
        
        Ok(())
    }
    
    /// Fetch bonding curve state from the blockchain
    async fn fetch_bonding_curve_state(&self, bonding_curve_address: &str) -> Result<Box<dyn BondingCurveStateTrait>> {
        // Convert string to Pubkey
        let pubkey = Pubkey::from_str(bonding_curve_address)
            .with_context(|| format!("Invalid bonding curve address: {}", bonding_curve_address))?;
        
        // Fetch account data
        let account_data = self
            .rpc_client
            .get_account_data(&pubkey)
            .with_context(|| format!("Failed to get account data for {}", bonding_curve_address))?;
        
        // Parse state using the platform-specific logic
        let state = self
            .platform
            .parse_bonding_curve_state(&mut &*account_data)
            .with_context(|| format!("Failed to parse bonding curve state"))?;
        
        Ok(state)
    }
    
    /// Check for tokens close to graduation
    fn check_graduation_status(&self) {
        for token_entry in self.tokens.iter() {
            let token = token_entry.value();
            
            if token.is_graduated {
                continue; // Skip already graduated tokens
            }
            
            // Check if token is close to graduation based on progress thresholds
            if token.graduation_progress >= 90.0 {
                info!(
                    "[{}] Token {} is very close to graduation! Progress: {:.2}%, Needed: {:.2} SOL",
                    self.scan_id, token.mint_address, token.graduation_progress, token.needed_for_graduation
                );
            } else if token.graduation_progress >= 80.0 {
                info!(
                    "[{}] Token {} is approaching graduation. Progress: {:.2}%, Needed: {:.2} SOL",
                    self.scan_id, token.mint_address, token.graduation_progress, token.needed_for_graduation
                );
            }
        }
    }
    
    /// Log scanner statistics
    fn log_statistics(&self) {
        let tokens_count = self.tokens.len();
        let graduated_count = self.tokens.iter().filter(|t| t.value().is_graduated).count();
        
        info!(
            "[{}] Statistics: {} blocks, {} transactions, {} platform transactions processed",
            self.scan_id, self.blocks_processed, self.transactions_processed, self.platform_transactions_processed
        );
        
        info!(
            "[{}] Tracking {} tokens, {} graduated",
            self.scan_id, tokens_count, graduated_count
        );
    }
} 