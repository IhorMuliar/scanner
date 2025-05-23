mod config;
mod scanner;
mod utils;
mod types;

use anyhow::Result;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::scanner::TokenScanner;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solana_token_scanner=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    info!("🚀 Starting Solana Token Scanner...");

    // Run the main scanner logic
    if let Err(e) = run_scanner().await {
        error!("❌ Fatal error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_scanner() -> Result<()> {
    // Initialize the scanner
    let mut scanner = TokenScanner::new().await?;
    
    // Handle shutdown signals gracefully
    tokio::select! {
        result = scanner.start_scanning() => {
            if let Err(e) = result {
                error!("Scanner error: {}", e);
            }
        }
        _ = signal::ctrl_c() => {
            info!("⚠️ Received CTRL+C. Shutting down gracefully...");
            scanner.stop().await;
        }
    }

    info!("🛑 Scanner stopped.");
    Ok(())
} 