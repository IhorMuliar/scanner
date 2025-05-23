use serde::{Deserialize, Serialize};
use solana_sdk::commitment_config::CommitmentConfig;
use std::time::Duration;

/// Main configuration for the Solana Token Scanner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Solana RPC endpoint URL
    pub solana_rpc_url: String,
    
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            solana_rpc_url: std::env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            scan_interval_ms: 10_000, // 10 seconds
            max_blocks_to_process: 10,
            volume_threshold: 1.0, // SOL
            buyers_threshold: 1,
            age_threshold_minutes: 15,
            cleanup_interval_ms: 60_000, // 1 minute
            cleanup_age_multiplier: 2,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::default()
    }
    
    pub fn scan_interval(&self) -> Duration {
        Duration::from_millis(self.scan_interval_ms)
    }
    
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_millis(self.cleanup_interval_ms)
    }
}

/// Connection configuration for Solana RPC
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub commitment: CommitmentConfig,
    pub timeout: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            commitment: CommitmentConfig::confirmed(),
            timeout: Duration::from_secs(60),
        }
    }
}

/// Output formatting configuration
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
            hot_token_prefix: "[HOT]",
            separator: " | ",
            volume_label: "Volume: ",
            buyers_label: "Buyers: ",
            age_label: "Age: ",
            platform_label: "Platform: ",
            age_unit: " min",
        }
    }
} 