use tokio::signal;
use log::{error, info};
use anyhow::Result;

mod config;
mod scanner;
mod utils;

use crate::scanner::TokenScanner;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();
    
    info!("🚀 Starting Solana Token Scanner (Rust)...");
    
    // Create scanner instance
    let mut scanner = TokenScanner::new().await?;
    
    // Initialize scanner
    scanner.initialize().await?;
    
    // Set up graceful shutdown
    let scanner_clone = scanner.clone();
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to listen for ctrl-c: {}", e);
        }
        info!("⚠️ Received SIGINT. Shutting down gracefully...");
        scanner_clone.stop().await;
    });
    
    // Start scanning
    match scanner.start_scanning().await {
        Ok(_) => info!("Scanner finished successfully"),
        Err(e) => {
            error!("❌ Fatal error: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
} 