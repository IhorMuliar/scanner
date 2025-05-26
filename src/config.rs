use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::commitment_config::CommitmentConfig;

/// Scanner configuration settings
pub struct Config;

impl Config {
    // Solana RPC endpoint URL
    pub const SOLANA_RPC_URL: &'static str = "https://api.mainnet-beta.solana.com";
    
    // Scanning parameters
    pub const SCAN_INTERVAL_MS: u64 = 10000; // 10 seconds between scan cycles
    pub const MAX_BLOCKS_TO_PROCESS: u64 = 10; // Maximum blocks to process in one batch
    
    // Spike detection thresholds
    pub const VOLUME_THRESHOLD: f64 = 1.0; // Minimum volume in SOL
    pub const BUYERS_THRESHOLD: usize = 1; // Minimum unique buyers
    pub const AGE_THRESHOLD_MINUTES: u64 = 15; // Maximum age to be considered "new"
    
    // Memory management
    pub const CLEANUP_INTERVAL_MS: u64 = 60000; // How often to clean up old tokens (1 minute)
    pub const CLEANUP_AGE_MULTIPLIER: u64 = 2; // Multiple of AGE_THRESHOLD to keep tokens in memory
}

/// Connection parameters for Solana web3
pub fn get_connection_config() -> CommitmentConfig {
    CommitmentConfig::confirmed()
}

/// Block fetch configuration
pub fn get_block_config() -> RpcBlockConfig {
    RpcBlockConfig {
        encoding: None,
        transaction_details: Some(solana_transaction_status::TransactionDetails::Full),
        rewards: Some(false),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    }
}

/// Console output formatting
pub struct OutputFormat;

impl OutputFormat {
    pub const HOT_TOKEN_PREFIX: &'static str = "[HOT]";
    pub const SEPARATOR: &'static str = " | ";
    pub const VOLUME_LABEL: &'static str = "Volume: ";
    pub const BUYERS_LABEL: &'static str = "Buyers: ";
    pub const AGE_LABEL: &'static str = "Age: ";
    pub const PLATFORM_LABEL: &'static str = "Platform: ";
    pub const AGE_UNIT: &'static str = " min";
} 