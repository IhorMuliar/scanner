use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use log::{error, info};
use solana_token_scanner::{Config, TokenScanner};

/// Solana Token Scanner CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Solana RPC URL
    #[arg(long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    /// Yellowstone gRPC URL
    #[arg(long, default_value = "http://localhost:10000")]
    grpc_url: String,

    /// Scan interval in milliseconds
    #[arg(long, default_value_t = 10000)]
    scan_interval: u64,

    /// Volume threshold in SOL for spike detection
    #[arg(long, default_value_t = 1.0)]
    volume_threshold: f64,

    /// Minimum unique buyers for spike detection
    #[arg(long, default_value_t = 1)]
    buyers_threshold: usize,

    /// Maximum token age in minutes to consider for spikes
    #[arg(long, default_value_t = 15)]
    age_threshold: u64,

    /// Maximum number of blocks to process in one batch
    #[arg(long, default_value_t = 10)]
    max_blocks: u64,

    /// Transaction signature cache size for de-duplication
    #[arg(long, default_value_t = 10000)]
    tx_cache_size: usize,
}

/// Main entry point for the Solana Token Scanner
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Parse command line arguments
    let args = Args::parse();

    // Create configuration from CLI args
    let config = Config {
        solana_rpc_url: args.rpc_url,
        grpc_url: args.grpc_url,
        scan_interval_ms: args.scan_interval,
        max_blocks_to_process: args.max_blocks,
        volume_threshold: args.volume_threshold,
        buyers_threshold: args.buyers_threshold,
        age_threshold_minutes: args.age_threshold,
        cleanup_interval_ms: 60000,
        cleanup_age_multiplier: 2,
        tx_cache_size: args.tx_cache_size,
    };

    // Create and initialize scanner
    let scanner = TokenScanner::new(config);
    
    if let Err(e) = scanner.initialize().await {
        error!("❌ Failed to initialize scanner: {}", e);
        return Err(e);
    }

    // Start scanning
    info!("🚀 Starting Solana Token Scanner...");
    
    if let Err(e) = scanner.start_scanning().await {
        error!("❌ Fatal error during scanning: {}", e);
        return Err(e);
    }

    info!("✅ Scanner shutdown complete");
    Ok(())
} 