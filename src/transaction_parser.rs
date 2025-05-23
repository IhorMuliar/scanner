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

            // Extract signature from the transaction for reference
            let signature = tx.transaction.signatures.get(0)
                .map(|sig| sig.to_string())
                .unwrap_or_else(|| format!("unknown_{}", slot));

            // Process token transfers in the transaction
            if let Some(token_transfers) = &meta.pre_token_balances.as_ref().zip(meta.post_token_balances.as_ref()) {
                let (pre_balances, post_balances) = token_transfers;
                
                // Look for token balance changes
                for post_balance in post_balances {
                    // Find corresponding pre-balance
                    let pre_balance = pre_balances.iter()
                        .find(|pre| pre.owner == post_balance.owner && pre.mint == post_balance.mint);
                    
                    // If we found a matching pre-balance, check for a significant change
                    if let Some(pre_balance) = pre_balance {
                        // Only process if this is a real token transfer (not just a small fee)
                        if let (Some(pre_ui), Some(post_ui)) = (&pre_balance.ui_token_amount, &post_balance.ui_token_amount) {
                            if post_ui.ui_amount.unwrap_or(0.0) > pre_ui.ui_amount.unwrap_or(0.0) {
                                // This address received tokens
                                if let (Some(buyer_str), Some(mint_str)) = (&post_balance.owner, &post_balance.mint) {
                                    // Extract buyer and mint addresses
                                    if let (Ok(buyer), Ok(mint)) = (Pubkey::from_str(buyer_str), Pubkey::from_str(mint_str)) {
                                        // Calculate the SOL amount involved in the transaction (from fee payer)
                                        let sol_amount = self.extract_sol_amount(tx).unwrap_or(0.0);
                                        if sol_amount > 0.01 {
                                            // Determine which DEX program was used
                                            let program_id = self.identify_program_id(tx);
                                            
                                            // Create a token event
                                            let event = TokenEvent {
                                                mint,
                                                buyer,
                                                sol_amount,
                                                timestamp: block_time as u64,
                                                slot,
                                                signature: signature.clone(),
                                                program_id,
                                                event_type: self.determine_event_type(tx, sol_amount),
                                            };
                                            events.push(event);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
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
    
    fn identify_program_id(&self, tx: &EncodedTransactionWithStatusMeta) -> Pubkey {
        // Try to identify which DEX program is used in the transaction
        if let Some(meta) = &tx.meta {
            if let Some(log_messages) = &meta.log_messages {
                for log in log_messages {
                    if log.contains(PUMP_FUN_PROGRAM_ID) {
                        return self.pump_fun_program;
                    } else if log.contains(RAYDIUM_AMM_PROGRAM_ID) {
                        return self.raydium_program;
                    } else if log.contains(METEORA_PROGRAM_ID) {
                        return self.meteora_program;
                    }
                }
            }
            
            // If we couldn't identify from logs, check for program invocations in instructions
            if let Some(instructions) = &meta.inner_instructions {
                for inner_instruction_set in instructions {
                    for instruction in &inner_instruction_set.instructions {
                        if let Some(program_id) = &instruction.program_id {
                            if program_id == PUMP_FUN_PROGRAM_ID {
                                return self.pump_fun_program;
                            } else if program_id == RAYDIUM_AMM_PROGRAM_ID {
                                return self.raydium_program;
                            } else if program_id == METEORA_PROGRAM_ID {
                                return self.meteora_program;
                            }
                        }
                    }
                }
            }
        }
        
        // Default to Pump.fun if we can't determine
        self.pump_fun_program
    }
    
    fn determine_event_type(&self, tx: &EncodedTransactionWithStatusMeta, sol_amount: f64) -> EventType {
        // Try to determine the type of event from transaction data
        if let Some(meta) = &tx.meta {
            if let Some(log_messages) = &meta.log_messages {
                for log in log_messages {
                    if log.contains("mint") || log.contains("Mint") {
                        return EventType::TokenMint;
                    } else if log.contains("swap") || log.contains("Swap") {
                        return EventType::Swap;
                    } else if log.contains("liquidity") || log.contains("Liquidity") || log.contains("LP") {
                        return EventType::LpAction;
                    }
                }
            }
        }
        
        // Use SOL amount as a heuristic if we can't determine from logs
        if sol_amount > 1.0 {
            EventType::Buy
        } else {
            EventType::TokenMint
        }
    }
} 