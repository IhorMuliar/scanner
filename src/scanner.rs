use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::time::{sleep, Duration};
use solana_client::rpc_client::RpcClient;
use solana_transaction_status::UiConfirmedBlock;
use log::{info, error};
use anyhow::Result;

use crate::config::{Config, get_connection_config, get_block_config};
use crate::utils::{ProgramIds, determine_platform, TokenInfo, create_token_info};

/// Solana Token Scanner
/// Monitors transactions on Solana for token trading activity and detects spikes
#[derive(Clone)]
pub struct TokenScanner {
    connection: Arc<RpcClient>,
    is_scanning: Arc<AtomicBool>,
    last_processed_slot: Arc<AtomicU64>,
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
            ProgramIds::RAYDIUM_AMM,
            ProgramIds::METEORA,
        ];

        let relevant_program_index = account_keys.iter().position(|key| program_ids.contains(&key.as_str()));
        
        if let Some(program_index) = relevant_program_index {
            // Extract token information from the transaction
            if let Some(token_info) = self.extract_token_info(tx, program_index, &account_keys) {
                info!("💰 Found token activity: {:?}", token_info);
            }
        }
    }

    /// Extracts token information from a transaction
    fn extract_token_info(&self, tx: &solana_transaction_status::EncodedTransactionWithStatusMeta, program_index: usize, account_keys: &[String]) -> Option<TokenInfo> {
        let meta = tx.meta.as_ref()?;
        
        // Get program ID
        let program_id = account_keys.get(program_index)?.clone();
        
        // For now, create a placeholder token mint address
        // In a real implementation, you would parse the instruction data
        let token_mint_address = format!("{}...{}", &program_id[..8], &program_id[program_id.len()-8..]);

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

    /// Stops the scanner
    pub async fn stop(&self) {
        self.is_scanning.store(false, Ordering::SeqCst);
        info!("🛑 Scanner stopped.");
    }
} 