use anyhow::Result;
use log::{info, error, warn};
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::mpsc;

mod block_fetcher;
mod transaction_parser;
mod token_tracker;
mod spike_detector;
mod console_output;
mod grpc_stream;
mod deduplication;

use block_fetcher::BlockFetcher;
use transaction_parser::TransactionParser;
use token_tracker::TokenTracker;
use spike_detector::SpikeDetector;
use console_output::ConsoleOutput;
use grpc_stream::{GrpcStream, TransactionUpdate};
use deduplication::TransactionDeduplicator;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let console_output = ConsoleOutput::new();
    console_output.print_startup_banner();
    
    info!("Starting Solana Scanner with real-time streaming...");
    
    // Configuration from environment variables
    let rpc_url = env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    
    let grpc_url = env::var("YELLOWSTONE_GRPC_URL")
        .unwrap_or_else(|_| "http://grpc.solana.com:10000".to_string());
    
    let use_grpc = env::var("USE_GRPC").unwrap_or_else(|_| "true".to_string()) == "true";
    
    info!("Configuration:");
    info!("  RPC URL: {}", rpc_url);
    info!("  gRPC URL: {}", grpc_url);
    info!("  Use gRPC: {}", use_grpc);
    
    // Initialize components
    let block_fetcher = BlockFetcher::new(&rpc_url)?;
    let transaction_parser = TransactionParser::new();
    let mut token_tracker = TokenTracker::new();
    let mut spike_detector = SpikeDetector::new();
    let deduplicator = TransactionDeduplicator::new(10000); // 10k transaction cache
    
    if use_grpc {
        run_streaming_mode(
            grpc_url,
            block_fetcher,
            transaction_parser,
            token_tracker,
            spike_detector,
            console_output,
            deduplicator,
        ).await
    } else {
        run_polling_mode(
            block_fetcher,
            transaction_parser,
            token_tracker,
            spike_detector,
            console_output,
        ).await
    }
}

async fn run_streaming_mode(
    grpc_url: String,
    block_fetcher: BlockFetcher,
    transaction_parser: TransactionParser,
    mut token_tracker: TokenTracker,
    mut spike_detector: SpikeDetector,
    console_output: ConsoleOutput,
    deduplicator: TransactionDeduplicator,
) -> Result<()> {
    info!("Starting streaming mode with gRPC...");
    
    let mut grpc_stream = GrpcStream::new(&grpc_url).await?;
    let mut transaction_receiver = grpc_stream.subscribe_transactions().await?;
    
    let mut last_processed_slot = 0u64;
    let mut total_events_processed = 0usize;
    let mut scan_count = 0u64;
    
    // Spawn a task for periodic status monitoring
    let stats_dedup = deduplicator.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            
            let dedup_stats = stats_dedup.get_stats();
            info!("Deduplication cache: {}/{} entries ({}%)", 
                  dedup_stats.current_entries, 
                  dedup_stats.capacity,
                  dedup_stats.usage_percentage());
        }
    });
    
    loop {
        scan_count += 1;
        
        match run_streaming_cycle(
            &mut transaction_receiver,
            &block_fetcher,
            &transaction_parser,
            &mut token_tracker,
            &mut spike_detector,
            &console_output,
            &deduplicator,
            &mut last_processed_slot,
            &mut total_events_processed,
        ).await {
            Ok(hot_tokens_found) => {
                if scan_count % 10 == 0 {
                    console_output.print_scan_summary(
                        token_tracker.token_count(),
                        total_events_processed,
                        hot_tokens_found
                    );
                }
            },
            Err(e) => {
                console_output.print_error(&format!("Streaming cycle error: {}", e));
                error!("Error in streaming cycle: {}", e);
                
                // Try to reconnect
                if let Err(reconnect_err) = grpc_stream.reconnect().await {
                    error!("Failed to reconnect: {}", reconnect_err);
                    sleep(Duration::from_secs(5)).await;
                } else {
                    transaction_receiver = grpc_stream.subscribe_transactions().await?;
                }
            }
        }
        
        sleep(Duration::from_millis(100)).await; // Small delay to prevent tight loops
    }
}

async fn run_streaming_cycle(
    transaction_receiver: &mut mpsc::Receiver<TransactionUpdate>,
    block_fetcher: &BlockFetcher,
    transaction_parser: &TransactionParser,
    token_tracker: &mut TokenTracker,
    spike_detector: &mut SpikeDetector,
    console_output: &ConsoleOutput,
    deduplicator: &TransactionDeduplicator,
    last_processed_slot: &mut u64,
    total_events_processed: &mut usize,
) -> Result<usize> {
    let mut hot_token_count = 0;
    let mut updates_processed = 0;
    
    // Process available updates (non-blocking)
    while let Ok(update) = transaction_receiver.try_recv() {
        match update {
            TransactionUpdate::Transaction(tx_update) => {
                if let Some(signature) = update.get_signature() {
                    let slot = update.get_slot().unwrap_or(0);
                    
                    // Check deduplication for live transactions
                    if deduplicator.mark_live_transaction(&signature, slot) {
                        // Process this live transaction
                        if let Some(ref tx) = tx_update.transaction {
                            // Convert gRPC transaction to our format and parse
                            // This is a simplified conversion - you might need to adapt based on your parser
                            // For now, we'll skip live transaction processing and rely on confirmed blocks
                            info!("Live transaction received: {} at slot {}", signature, slot);
                        }
                    }
                }
            }
            TransactionUpdate::Block(block_update) => {
                let slot = block_update.slot;
                info!("Processing confirmed block: slot {}", slot);
                
                if slot > *last_processed_slot {
                    // Get full block data for parsing
                    if let Some(block) = block_fetcher.get_block(slot).await? {
                        let token_events = transaction_parser.parse_block(&block)?;
                        
                        // Process events with deduplication
                        for event in token_events {
                            if let Some(signature) = &event.signature {
                                if deduplicator.should_process_block_transaction(signature, slot) {
                                    // Process the event
                                    token_tracker.update(&event);
                                    *total_events_processed += 1;
                                    
                                    // Mark as processed
                                    deduplicator.mark_block_transaction(signature, slot);
                                }
                            }
                        }
                        
                        console_output.print_status_line(slot, token_tracker.token_count());
                        *last_processed_slot = slot;
                    }
                }
            }
        }
        
        updates_processed += 1;
        if updates_processed >= 50 { // Limit per cycle to prevent blocking
            break;
        }
    }
    
    // Check for spikes and output hot tokens
    let hot_tokens = spike_detector.detect_hot_tokens(&token_tracker);
    hot_token_count = hot_tokens.len();
    
    for hot_token in hot_tokens {
        console_output.print_hot_token(&hot_token);
    }
    
    if hot_token_count > 0 {
        println!(); // Clear the status line
    }
    
    Ok(hot_token_count)
}

async fn run_polling_mode(
    block_fetcher: BlockFetcher,
    transaction_parser: TransactionParser,
    mut token_tracker: TokenTracker,
    mut spike_detector: SpikeDetector,
    console_output: ConsoleOutput,
) -> Result<()> {
    info!("Starting polling mode (legacy RPC)...");
    
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
        
        sleep(Duration::from_millis(400)).await;
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
    let latest_slot = block_fetcher.get_latest_slot().await?;
    
    if latest_slot <= *last_processed_slot {
        return Ok(0);
    }
    
    let start_slot = (*last_processed_slot + 1).max(latest_slot.saturating_sub(5));
    
    for slot in start_slot..=latest_slot {
        if let Some(block) = block_fetcher.get_block(slot).await? {
            info!("Processing slot {}", slot);
            
            let token_events = transaction_parser.parse_block(&block)?;
            *total_events_processed += token_events.len();
            
            for event in token_events {
                token_tracker.update(&event);
            }
            
            console_output.print_status_line(slot, token_tracker.token_count());
        }
        
        *last_processed_slot = slot;
    }
    
    let hot_tokens = spike_detector.detect_hot_tokens(&token_tracker);
    let hot_token_count = hot_tokens.len();
    
    for hot_token in hot_tokens {
        console_output.print_hot_token(&hot_token);
    }
    
    if hot_token_count > 0 {
        println!();
    }
    
    Ok(hot_token_count)
} 