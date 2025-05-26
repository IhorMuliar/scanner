use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::time::{sleep, Duration};
use solana_client::rpc_client::RpcClient;
use log::{info, error};
use anyhow::Result;

use crate::config::{Config, get_connection_config, get_block_config};

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

    /// Processes a single block, analyzing its basic information
    async fn process_block(&self, slot: u64) -> Result<()> {
        match self.connection.get_block_with_config(slot, get_block_config()) {
            Ok(block) => {
                let tx_count = block.transactions.as_ref().map(|txs| txs.len()).unwrap_or(0);
                let block_time = block.block_time.unwrap_or(0);
                
                info!("🧱 Block {}: {} transactions, block_time: {}", slot, tx_count, block_time);
                
                // For step 1, we only display block info, no transaction processing yet
                Ok(())
            }
            Err(e) => {
                error!("❌ Error processing block {}: {}", slot, e);
                // Don't crash on block fetch errors, just log and continue
                Ok(())
            }
        }
    }

    /// Stops the scanner
    pub async fn stop(&self) {
        self.is_scanning.store(false, Ordering::SeqCst);
        info!("🛑 Scanner stopped.");
    }
} 