use serde::{Deserialize, Serialize};
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use std::time::Duration;

/// Scanner configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Solana RPC endpoint URL
    pub solana_rpc_url: String,
    
    /// gRPC Yellowstone Geyser endpoint URL
    pub grpc_url: String,
    
    /// Scanning parameters
    pub scan_interval_ms: u64,
    pub max_blocks_to_process: u64,
    
    /// Spike detection thresholds
    pub volume_threshold: f64,
    pub buyers_threshold: usize,
    pub age_threshold_minutes: u64,
    
    /// Memory management
    pub cleanup_interval_ms: u64,
    pub cleanup_age_multiplier: u64,
    
    /// De-duplication settings
    pub tx_cache_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            solana_rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            grpc_url: "http://localhost:10000".to_string(),
            scan_interval_ms: 10000, // 10 seconds
            max_blocks_to_process: 10,
            volume_threshold: 1.0, // SOL
            buyers_threshold: 1,
            age_threshold_minutes: 15,
            cleanup_interval_ms: 60000, // 1 minute
            cleanup_age_multiplier: 2,
            tx_cache_size: 10000, // LRU cache size for transaction de-duplication
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn scan_interval(&self) -> Duration {
        Duration::from_millis(self.scan_interval_ms)
    }
    
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_millis(self.cleanup_interval_ms)
    }
}

/// Connection configuration for Solana client
pub struct ConnectionConfig;

impl ConnectionConfig {
    pub fn commitment_config() -> CommitmentConfig {
        CommitmentConfig::confirmed()
    }
    
    pub fn timeout() -> Duration {
        Duration::from_secs(60)
    }
}

/// Block fetch configuration
pub struct BlockConfig;

impl BlockConfig {
    pub fn rpc_block_config() -> RpcBlockConfig {
        RpcBlockConfig {
            encoding: None,
            transaction_details: Some(solana_transaction_status::TransactionDetails::Full),
            rewards: Some(false),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        }
    }
}

/// Console output formatting
#[derive(Debug, Clone)]
pub struct OutputFormat {
    pub hot_token_prefix: &'static str,
    pub separator: &'static str,
    pub volume_label: &'static str,
    pub buyers_label: &'static str,
    pub age_label: &'static str,
    pub platform_label: &'static str,
    pub age_unit: &'static str,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self {
            hot_token_prefix: "🔥 [HOT]",
            separator: " | ",
            volume_label: "Volume: ",
            buyers_label: "Buyers: ",
            age_label: "Age: ",
            platform_label: "Platform: ",
            age_unit: " min",
        }
    }
} 