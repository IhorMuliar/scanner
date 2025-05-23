use anyhow::Result;
use solana_transaction_status::{UiConfirmedBlock, EncodedTransactionWithStatusMeta};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use log::debug;
use std::str::FromStr;

// Known program IDs for DEXs we're monitoring
pub const PUMP_FUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"; // Pump.fun
pub const RAYDIUM_AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"; // Raydium AMM
// LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo ?
pub const METEORA_PROGRAM_ID: &str = "24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi"; // Meteora

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEvent {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub sol_amount: f64,
    pub timestamp: u64,
    pub slot: u64,
    pub signature: String,
    pub program_id: Pubkey,
    pub event_type: EventType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    TokenMint,
    Swap,
    Buy,
    LpAction,
}

pub struct TransactionParser {
    pump_fun_program: Pubkey,
    raydium_program: Pubkey,
    meteora_program: Pubkey,
}

impl TransactionParser {
    pub fn new() -> Self {
        Self {
            pump_fun_program: Pubkey::from_str(PUMP_FUN_PROGRAM_ID).unwrap(),
            raydium_program: Pubkey::from_str(RAYDIUM_AMM_PROGRAM_ID).unwrap(),
            meteora_program: Pubkey::from_str(METEORA_PROGRAM_ID).unwrap(),
        }
    }
    
    pub fn parse_block(&self, block: &UiConfirmedBlock) -> Result<Vec<TokenEvent>> {
        let mut events = Vec::new();
        
        if let Some(transactions) = &block.transactions {
            for tx in transactions {
                if let Some(parsed_events) = self.parse_transaction(tx, block.block_time.unwrap_or(0), block.parent_slot) {
                    events.extend(parsed_events);
                }
            }
        }
        
        debug!("Parsed {} token events from block", events.len());
        Ok(events)
    }
    
    fn parse_transaction(
        &self, 
        tx: &EncodedTransactionWithStatusMeta,
        block_time: i64,
        slot: u64
    ) -> Option<Vec<TokenEvent>> {
        let mut events = Vec::new();
        
        // Check if transaction was successful
        if let Some(meta) = &tx.meta {
            if meta.err.is_some() {
                return None; // Skip failed transactions
            }
        }
        
        // For MVP, we'll create synthetic events from any transactions 
        // that have balance changes, indicating potential token activity
        if let Some(sol_amount) = self.extract_sol_amount(tx) {
            if sol_amount > 0.01 { // Only process if meaningful SOL amount
                // Generate a placeholder event for demonstration
                let event = TokenEvent {
                    mint: self.generate_placeholder_mint(),
                    buyer: self.generate_placeholder_buyer(),
                    sol_amount,
                    timestamp: block_time as u64,
                    slot,
                    signature: format!("synthetic_{}", slot),
                    program_id: self.pump_fun_program, // Default to pump.fun for demo
                    event_type: if sol_amount > 1.0 { EventType::Buy } else { EventType::TokenMint },
                };
                events.push(event);
            }
        }
        
        if !events.is_empty() {
            Some(events)
        } else {
            None
        }
    }
    
    fn extract_sol_amount(&self, tx: &EncodedTransactionWithStatusMeta) -> Option<f64> {
        if let Some(meta) = &tx.meta {
            let pre_balances = &meta.pre_balances;
            let post_balances = &meta.post_balances;
            
            // Look for the largest balance change (typically the buyer)
            let mut max_change = 0i64;
            for (pre, post) in pre_balances.iter().zip(post_balances.iter()) {
                let change = (*post as i64) - (*pre as i64);
                if change.abs() > max_change.abs() {
                    max_change = change;
                }
            }
            
            if max_change.abs() > 0 {
                return Some((max_change.abs() as f64) / 1_000_000_000.0); // Convert lamports to SOL
            }
        }
        
        None // Return None if no balance change detected
    }
    
    fn generate_placeholder_mint(&self) -> Pubkey {
        // Generate a different placeholder mint each time for demonstration
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut mint_bytes = [0u8; 32];
        let timestamp_bytes = timestamp.to_le_bytes();
        mint_bytes[..8].copy_from_slice(&timestamp_bytes);
        Pubkey::new_from_array(mint_bytes)
    }
    
    fn generate_placeholder_buyer(&self) -> Pubkey {
        // Generate a different placeholder buyer
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mut buyer_bytes = [0u8; 32];
        let timestamp_bytes = (timestamp as u64).to_le_bytes();
        buyer_bytes[..8].copy_from_slice(&timestamp_bytes);
        buyer_bytes[8] = 1; // Make it different from mint
        Pubkey::new_from_array(buyer_bytes)
    }
} 