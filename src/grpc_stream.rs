use anyhow::Result;
use log::{info, error, warn};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientError};
use yellowstone_grpc_proto::prelude::*;
use solana_sdk::signature::Signature;
use solana_transaction_status::UiConfirmedBlock;

use crate::transaction_parser::TokenEvent;

pub struct GrpcStream {
    client: GeyserGrpcClient<tonic::transport::channel::Channel>,
    endpoint: String,
}

impl GrpcStream {
    pub async fn new(endpoint: &str) -> Result<Self> {
        info!("Connecting to gRPC endpoint: {}", endpoint);
        
        let client = GeyserGrpcClient::connect(
            endpoint,
            Some("x-token".to_string()),
            None,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to gRPC: {}", e))?;
        
        info!("Successfully connected to Yellowstone gRPC");
        
        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
        })
    }
    
    pub async fn subscribe_transactions(&mut self) -> Result<mpsc::Receiver<TransactionUpdate>> {
        let mut accounts = HashMap::new();
        let mut slots = HashMap::new();
        let mut transactions = HashMap::new();
        let mut blocks = HashMap::new();
        let mut blocks_meta = HashMap::new();
        let mut entry = HashMap::new();
        
        // Subscribe to transactions with our filter criteria
        transactions.insert(
            "client".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include: vec![
                    // Pump.fun program ID
                    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string(),
                    // Raydium AMM program ID  
                    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
                    // Meteora program ID
                    "24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi".to_string(),
                ],
                account_exclude: vec![],
                account_required: vec![],
            },
        );
        
        // Subscribe to block confirmations as well
        blocks.insert(
            "client".to_string(), 
            SubscribeRequestFilterBlocks {
                account_include: vec![],
                include_transactions: Some(true),
                include_accounts: Some(false),
                include_entries: Some(false),
            },
        );
        
        let subscribe_request = SubscribeRequest {
            accounts,
            slots,
            transactions,
            blocks,
            blocks_meta,
            entry,
            commitment: Some(CommitmentLevel::Confirmed as i32),
            accounts_data_slice: vec![],
            ping: None,
        };
        
        info!("Starting gRPC subscription...");
        
        let (geyser_tx, geyser_rx) = mpsc::channel(1000);
        let (tx_update_tx, tx_update_rx) = mpsc::channel(1000);
        
        // Subscribe to the stream
        match self.client.subscribe_with_request(Some(subscribe_request)).await {
            Ok(mut stream) => {
                info!("Successfully subscribed to gRPC stream");
                
                // Spawn task to handle the gRPC stream
                tokio::spawn(async move {
                    while let Some(message) = stream.next().await {
                        match message {
                            Ok(msg) => {
                                if let Err(e) = geyser_tx.send(msg).await {
                                    error!("Failed to send geyser message: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("gRPC stream error: {}", e);
                                break;
                            }
                        }
                    }
                    warn!("gRPC stream ended");
                });
                
                // Spawn task to process geyser messages
                tokio::spawn(async move {
                    let mut geyser_rx = geyser_rx;
                    while let Some(msg) = geyser_rx.recv().await {
                        if let Some(update) = Self::process_geyser_message(msg) {
                            if let Err(e) = tx_update_tx.send(update).await {
                                error!("Failed to send transaction update: {}", e);
                                break;
                            }
                        }
                    }
                });
                
                Ok(tx_update_rx)
            }
            Err(e) => {
                error!("Failed to subscribe to gRPC stream: {}", e);
                Err(anyhow::anyhow!("gRPC subscription failed: {}", e))
            }
        }
    }
    
    fn process_geyser_message(msg: SubscribeUpdate) -> Option<TransactionUpdate> {
        match msg.update_oneof {
            Some(subscribe_update::UpdateOneof::Transaction(tx_update)) => {
                info!("Received live transaction: {}", 
                      tx_update.transaction.as_ref()
                          .and_then(|t| t.signature.as_ref())
                          .map(|s| bs58::encode(s).into_string())
                          .unwrap_or_else(|| "unknown".to_string())
                );
                
                Some(TransactionUpdate::Transaction(tx_update))
            }
            Some(subscribe_update::UpdateOneof::Block(block_update)) => {
                info!("Received confirmed block: slot {}", block_update.slot);
                Some(TransactionUpdate::Block(block_update))
            }
            Some(subscribe_update::UpdateOneof::Ping(_)) => {
                // Handle ping to keep connection alive
                None
            }
            _ => None,
        }
    }
    
    pub async fn reconnect(&mut self) -> Result<()> {
        warn!("Attempting to reconnect to gRPC endpoint...");
        
        for attempt in 1..=5 {
            match GeyserGrpcClient::connect(
                &self.endpoint,
                Some("x-token".to_string()),
                None,
            ).await {
                Ok(client) => {
                    self.client = client;
                    info!("Successfully reconnected to gRPC on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    warn!("Reconnection attempt {} failed: {}", attempt, e);
                    tokio::time::sleep(Duration::from_secs(attempt * 2)).await;
                }
            }
        }
        
        Err(anyhow::anyhow!("Failed to reconnect after 5 attempts"))
    }
}

#[derive(Debug)]
pub enum TransactionUpdate {
    Transaction(SubscribeUpdateTransaction),
    Block(SubscribeUpdateBlock),
}

impl TransactionUpdate {
    pub fn get_signature(&self) -> Option<String> {
        match self {
            TransactionUpdate::Transaction(tx) => {
                tx.transaction.as_ref()
                    .and_then(|t| t.signature.as_ref())
                    .map(|s| bs58::encode(s).into_string())
            }
            TransactionUpdate::Block(_) => None,
        }
    }
    
    pub fn get_slot(&self) -> Option<u64> {
        match self {
            TransactionUpdate::Transaction(tx) => Some(tx.slot),
            TransactionUpdate::Block(block) => Some(block.slot),
        }
    }
    
    pub fn is_transaction(&self) -> bool {
        matches!(self, TransactionUpdate::Transaction(_))
    }
    
    pub fn is_block(&self) -> bool {
        matches!(self, TransactionUpdate::Block(_))
    }
} 