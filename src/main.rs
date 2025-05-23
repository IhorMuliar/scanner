use anyhow::Result;
use log::{info, error};
use std::time::Duration;
use tokio::time::sleep;

mod block_fetcher;
mod transaction_parser;
mod token_tracker;
mod spike_detector;
mod console_output;

use block_fetcher::BlockFetcher;
use transaction_parser::TransactionParser;
use token_tracker::TokenTracker;
use spike_detector::SpikeDetector;
use console_output::ConsoleOutput;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let console_output = ConsoleOutput::new();
    console_output.print_startup_banner();
    
    info!("Starting Solana Scanner...");
    
    let block_fetcher = BlockFetcher::new("https://api.mainnet-beta.solana.com")?;
    let transaction_parser = TransactionParser::new();
    let mut token_tracker = TokenTracker::new();
    let mut spike_detector = SpikeDetector::new();
    
    let mut last_processed_slot = 0u64;
    let mut scan_count = 0u64;
    let mut total_events_processed = 0usize;
    
    loop {
        scan_count += 1;
        
        match run_scan_cycle(
            &block_fetcher,
            &transaction_parser,
            &mut token_tracker,
            &mut spike_detector,
            &console_output,
            &mut last_processed_slot,
            &mut total_events_processed,
        ).await {
            Ok(hot_tokens_found) => {
                // Print periodic status updates
                if scan_count % 50 == 0 {
                    console_output.print_scan_summary(
                        token_tracker.token_count(),
                        total_events_processed,
                        hot_tokens_found
                    );
                }
            },
            Err(e) => {
                console_output.print_error(&format!("Scan cycle error: {}", e));
                error!("Error in scan cycle: {}", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
        
        sleep(Duration::from_millis(400)).await; // Poll every 400ms
    }
}

async fn run_scan_cycle(
    block_fetcher: &BlockFetcher,
    transaction_parser: &TransactionParser,
    token_tracker: &mut TokenTracker,
    spike_detector: &mut SpikeDetector,
    console_output: &ConsoleOutput,
    last_processed_slot: &mut u64,
    total_events_processed: &mut usize,
) -> Result<usize> {
    // Step 1: Get latest slot
    let latest_slot = block_fetcher.get_latest_slot().await?;
    
    if latest_slot <= *last_processed_slot {
        return Ok(0);
    }
    
    // Process any missed slots (up to 5 to avoid overload in MVP)
    let start_slot = (*last_processed_slot + 1).max(latest_slot.saturating_sub(5));
    let mut _events_this_cycle = 0;
    
    for slot in start_slot..=latest_slot {
        if let Some(block) = block_fetcher.get_block(slot).await? {
            info!("Processing slot {}", slot);
            
            // Step 2: Parse transactions and extract token events
            let token_events = transaction_parser.parse_block(&block)?;
            _events_this_cycle += token_events.len();
            *total_events_processed += token_events.len();
            
            // Step 3: Update token tracker
            for event in token_events {
                token_tracker.update(&event);
            }
            
            // Show progress for active scanning
            console_output.print_status_line(slot, token_tracker.token_count());
        }
        
        *last_processed_slot = slot;
    }
    
    // Step 4 & 5: Check for spikes and output hot tokens
    let hot_tokens = spike_detector.detect_hot_tokens(&token_tracker);
    let hot_token_count = hot_tokens.len();
    
    for hot_token in hot_tokens {
        console_output.print_hot_token(&hot_token);
    }
    
    // Clear status line if we found hot tokens
    if hot_token_count > 0 {
        println!(); // Clear the status line
    }
    
    Ok(hot_token_count)
} 