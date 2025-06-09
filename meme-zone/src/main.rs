// Import from the library crate
use meme_scanner::{
    platforms::{
        BoopFunStrategy,
        DynamicBondingCurveStrategy,
        MoonitStrategy,
        PumpFunStrategy,
        RaydiumLaunchlabStrategy
    },  
    SolanaBlockScanner, 
    TokenPlatformTrait
};

use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use log::{error, info, LevelFilter};
use std::time::Duration;

/// Command line arguments for the meme token scanner
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Solana RPC endpoint URL
    #[arg(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    /// Platform to monitor (pump-fun, raydium, moonit)
    #[arg(short, long, default_value = "pump-fun")]
    platform: String,

    /// Start slot for scanning (0 = latest)
    #[arg(short, long, default_value_t = 0)]
    start_slot: u64,

    /// Maximum number of blocks to process
    #[arg(short, long, default_value_t = 10000)]
    max_blocks: u64,

    /// Polling interval in milliseconds
    #[arg(short, long, default_value_t = 500)]
    interval_ms: u64,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if present
    dotenv().ok();

    // Parse command line arguments
    let args = Args::parse();

    // Configure logging
    let log_level = match args.log_level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    env_logger::builder()
        .format_timestamp_millis()
        .filter_level(log_level)
        .init();

    // Determine start slot
    let start_slot = if args.start_slot == 0 {
        let rpc_client = solana_client::rpc_client::RpcClient::new(args.rpc_url.clone());
        match rpc_client.get_slot() {
            Ok(slot) => slot,
            Err(e) => {
                error!("Failed to get latest slot: {}", e);
                return Err(anyhow::anyhow!("Failed to get latest slot"));
            }
        }
    } else {
        args.start_slot
    };

    // Select platform strategy based on args
    let platform: Box<dyn TokenPlatformTrait> = match args.platform.as_str() {
        "boop-fun" => Box::new(BoopFunStrategy::new()),
        "dynamic-bonding-curve" => Box::new(DynamicBondingCurveStrategy::new()),
        "pump-fun" => Box::new(PumpFunStrategy::new()),
        "raydium" => Box::new(RaydiumLaunchlabStrategy::new()),
        "moonit" => Box::new(MoonitStrategy::new()),
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported platform: {}. Valid options are: pump-fun, raydium, moonit",
                args.platform
            ));
        }
    };

    info!("Starting {} scanner at slot {}", args.platform, start_slot);

    // Create and run scanner
    let mut scanner = SolanaBlockScanner::new(
        args.rpc_url,
        start_slot,
        Duration::from_millis(args.interval_ms),
        args.max_blocks,
        platform,
    )
    .await?;

    let data_encoded = "8B8g4BR2oTj";

    let data_decoded = bs58::decode(data_encoded).into_vec().unwrap();

    println!("Data decoded: {:?}", data_decoded);

    // Start scanning
    scanner.start_scanning().await?;

    Ok(())
} 