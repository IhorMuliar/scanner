use anyhow::{anyhow, Result};
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use std::fmt;

use crate::platform_trait::{BondingCurveStateTrait, InstructionType, TokenPlatformTrait};

/// Constants for Raydium Launchlab program
const RAYDIUM_LAUNCHLAB_PROGRAM_ID: &str = "LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj";
/// Instruction discriminators for Raydium Launch Lab program instructions
const BUY_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [250, 234, 13, 123, 213, 156, 19, 236];
const SELL_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [149, 39, 222, 155, 211, 124, 152, 26];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const MIGRATE_AMM_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [207, 82, 192, 145, 254, 207, 145, 223];
const MIGRATE_CPSWAP_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [136, 92, 200, 103, 28, 218, 144, 140];
/// Bonding curve constants for graduation calculations
/// Initial virtual token reserves at the start of the curve (1,073,000,000 * 10^6)
const INITIAL_VIRTUAL_BASE: u64 = 1_073_000_000_000_000;
/// Total tokens that can be sold from the curve (793,100,000 * 10^6)
const TOTAL_SELLABLE_TOKENS: u64 = 793_100_000_000_000;
/// SOL threshold for graduation to Raydium (approximately 85 SOL)
const GRADUATION_QUOTE_THRESHOLD: f64 = 85.0;
/// Graduation progress thresholds for alerting
const GRADUATION_ALERT_THRESHOLDS: &[f64] = &[50.0, 70.0, 80.0, 90.0, 95.0, 99.0];

/// Bonding curve status constants
/// Active trading phase
const STATUS_ACTIVE: u8 = 0;
/// Completed/migrated to AMM
const STATUS_COMPLETED: u8 = 1;

/// Bonding curve state for Raydium Launchlab tokens
#[derive(Debug, BorshDeserialize)]
pub struct RaydiumLaunchlabBondingCurveState {
    epoch: u64,
    auth_bump: u8,
    status: u8,
    base_decimals: u8,
    quote_decimals: u8,
    migrate_type: u8,
    supply: u64,
    total_base_sell: u64,
    virtual_base: u64,
    virtual_quote: u64,
    real_base: u64,
    real_quote: u64,
    total_quote_fund_raising: u64,
    quote_protocol_fee: u64,
    platform_fee: u64,
    migrate_fee: u64,
}

impl BondingCurveStateTrait for RaydiumLaunchlabBondingCurveState {
    /// Calculate the graduation progress percentage (0-100)
    /// Based on: ((INITIAL_VIRTUAL_BASE - virtual_base) * 100) / TOTAL_SELLABLE_TOKENS
    fn calculate_graduation_progress(&self) -> f64 {
        // Using virtual_base instead of virtual_token_reserves
        if self.virtual_base >= INITIAL_VIRTUAL_BASE {
            return 0.0;
        }
        
        let tokens_sold = INITIAL_VIRTUAL_BASE - self.virtual_base;
        (tokens_sold as f64 * 100.0) / TOTAL_SELLABLE_TOKENS as f64
    }
    
    /// Calculate current market cap in quote token using bonding curve formula
    /// Using virtual reserves for accurate pricing: virtual_quote / virtual_base
    fn calculate_market_cap(&self) -> f64 {
        if self.virtual_base == 0 {
            return 0.0;
        }
        
        // Current price per token in quote token
        let price_per_token = self.virtual_quote as f64 / self.virtual_base as f64;
        
        // Market cap = price * circulating supply
        let circulating_supply = self.supply - self.real_base;
        (price_per_token * circulating_supply as f64) / (10u64.pow(self.quote_decimals as u32) as f64)
    }
    
    /// Calculate SOL needed to reach graduation threshold
    fn needed_for_graduation(&self) -> f64 {
        (GRADUATION_QUOTE_THRESHOLD - self.current_sol_amount()).max(0.0)
    }
    
    /// Check if token is close to graduation (above any threshold)
    fn is_close_to_graduation(&self) -> Option<f64> {
        let progress = self.calculate_graduation_progress();
        GRADUATION_ALERT_THRESHOLDS.iter()
            .find(|&&threshold| progress >= threshold)
            .copied()
    }
    
    /// Get current quote token amount
    fn current_sol_amount(&self) -> f64 {
        // Convert real_quote to the proper decimal representation
        self.real_quote as f64 / (10u64.pow(self.quote_decimals as u32) as f64)
    }
    
    /// Check if curve is complete/migrated
    fn is_complete(&self) -> bool {
        // Check if status indicates completion
        self.status == STATUS_COMPLETED || 
        // Also check if graduation threshold has been reached
        self.current_sol_amount() >= GRADUATION_QUOTE_THRESHOLD
    }
}

impl fmt::Display for RaydiumLaunchlabBondingCurveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RaydiumLaunchlabBondingCurve {{ progress: {:.2}%, cap: {:.2} SOL, needed: {:.2} SOL, status: {} }}",
            self.calculate_graduation_progress(),
            self.calculate_market_cap(),
            self.needed_for_graduation(),
            self.status
        )
    }
}

/// Strategy implementation for Raydium Launchlab platform
pub struct RaydiumLaunchlabStrategy;

impl RaydiumLaunchlabStrategy {
    pub fn new() -> Self {
        RaydiumLaunchlabStrategy
    }
}

impl TokenPlatformTrait for RaydiumLaunchlabStrategy {
    fn program_id(&self) -> &'static str {
        RAYDIUM_LAUNCHLAB_PROGRAM_ID
    }
    
    fn buy_instruction_discriminator(&self) -> &'static [u8; 8] {
        &BUY_INSTRUCTION_DISCRIMINATOR
    }
    
    fn sell_instruction_discriminator(&self) -> &'static [u8; 8] {
        &SELL_INSTRUCTION_DISCRIMINATOR
    }
    
    fn create_instruction_discriminator(&self) -> &'static [u8; 8] {
        &CREATE_INSTRUCTION_DISCRIMINATOR
    }
    
    fn migrate_instruction_discriminator(&self) -> &'static [u8; 8] {
        // Default to AMM migrate for general migrate calls
        &MIGRATE_AMM_INSTRUCTION_DISCRIMINATOR
    }
    
    fn migrate_amm_instruction_discriminator(&self) -> Option<&'static [u8; 8]> {
        Some(&MIGRATE_AMM_INSTRUCTION_DISCRIMINATOR)
    }
    
    fn migrate_cpswap_instruction_discriminator(&self) -> Option<&'static [u8; 8]> {
        Some(&MIGRATE_CPSWAP_INSTRUCTION_DISCRIMINATOR)
    }
    
    fn is_platform_transaction(&self, transaction: &EncodedTransactionWithStatusMeta) -> bool {
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // Check if Pump.fun program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == RAYDIUM_LAUNCHLAB_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Pump.fun program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == RAYDIUM_LAUNCHLAB_PROGRAM_ID)
                    }
                }
            }
            _ => {
                // For non-JSON formats, we can't easily check, so return false
                false
            }
        }
    }
    
    fn has_instruction_type(
        &self, 
        transaction: &EncodedTransactionWithStatusMeta,
        discriminator: &[u8; 8],
    ) -> bool {
        // Process instructions based on transaction type
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, we need to check each instruction 
                        // differently since they have a different structure
                        for instruction in &parsed_message.instructions {
                            // For parsed instructions, we need to get program_id and data
                            // differently based on whether it's a parsed instruction or not
                            let (_program_id, data) = match instruction {
                                solana_transaction_status::UiInstruction::Parsed(_parsed_instruction) => {
                                    // For parsed instructions, we don't have direct access to raw data
                                    // Let's skip them for now and rely on the compiled instructions
                                    continue;
                                },
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Get program ID from account keys and program_id_index
                                    let program_idx = compiled.program_id_index as usize;
                                    if program_idx >= parsed_message.account_keys.len() {
                                        continue;
                                    }

                                    let program_id = &parsed_message.account_keys[program_idx].pubkey;
                                    if program_id != RAYDIUM_LAUNCHLAB_PROGRAM_ID {
                                        continue;
                                    }

                                    (program_id, &compiled.data)
                                }
                            };

                            // Now check the instruction data for the discriminator
                            // Decode instruction data from base58
                            if let Ok(decoded_data) = bs58::decode(data).into_vec() {
                                // Check if data starts with our target discriminator
                                if decoded_data.len() >= 8 && decoded_data[0..8] == discriminator[..] {
                                    return true;
                                }
                            }
                        }
                        false
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Handle raw instructions
                        for instruction in &raw_message.instructions {
                            // For raw instructions, we need to map program_id index to the actual ID
                            let program_id = if instruction.program_id_index < raw_message.account_keys.len() as u8 {
                                &raw_message.account_keys[instruction.program_id_index as usize]
                            } else {
                                continue;
                            };
                            
                            if program_id == RAYDIUM_LAUNCHLAB_PROGRAM_ID {
                                // Decode instruction data from base58
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    // Check if data starts with our target discriminator
                                    if data.len() >= 8 && data[0..8] == discriminator[..] {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                }
            }
            _ => return false,
        }
    }
    
    fn determine_instruction_type(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<InstructionType> {
        if self.has_instruction_type(transaction, self.buy_instruction_discriminator()) {
            Some(InstructionType::Buy)
        } else if self.has_instruction_type(transaction, self.sell_instruction_discriminator()) {
            Some(InstructionType::Sell)
        } else if self.has_instruction_type(transaction, self.create_instruction_discriminator()) {
            Some(InstructionType::Create)
        } else if self.has_instruction_type(transaction, &MIGRATE_AMM_INSTRUCTION_DISCRIMINATOR) {
            Some(InstructionType::MigrateAmm)
        } else if self.has_instruction_type(transaction, &MIGRATE_CPSWAP_INSTRUCTION_DISCRIMINATOR) {
            Some(InstructionType::MigrateCpswap)
        } else {
            Some(InstructionType::Other)
        }
    }
    
    fn extract_mint_address(&self, transaction: &EncodedTransactionWithStatusMeta, instruction_type: &InstructionType) -> Option<String> {
        // Get the appropriate discriminator for the instruction type
        let discriminator = match instruction_type {
            InstructionType::Buy => &BUY_INSTRUCTION_DISCRIMINATOR,
            InstructionType::Sell => &SELL_INSTRUCTION_DISCRIMINATOR,
            InstructionType::Create => &CREATE_INSTRUCTION_DISCRIMINATOR,
            InstructionType::Migrate | InstructionType::MigrateAmm => &MIGRATE_AMM_INSTRUCTION_DISCRIMINATOR,
            InstructionType::MigrateCpswap => &MIGRATE_CPSWAP_INSTRUCTION_DISCRIMINATOR,
            _ => return None,
        };

        // Find the instruction with the matching discriminator
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, check each instruction
                        for instruction in &parsed_message.instructions {
                            match instruction {
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Check if this matches our target instruction type
                                    if let Ok(data) = bs58::decode(&compiled.data).into_vec() {
                                        if data.len() >= 8 && data[0..8] == discriminator[..] {
                                            // For different instruction types, the mint account is at different positions
                                            let mint_account_index = match instruction_type {
                                                InstructionType::Buy | InstructionType::Sell => {
                                                    // For buy/sell, mint is typically the 10th account (index 9)
                                                    if compiled.accounts.len() > 9 { Some(9) } else { None }
                                                }
                                                InstructionType::Create => {
                                                    // For create, mint is typically the 7th account (index 6)
                                                    if compiled.accounts.len() > 6 { Some(6) } else { None }
                                                }
                                                InstructionType::Migrate | InstructionType::MigrateAmm | InstructionType::MigrateCpswap => {
                                                    // For migrate, mint is typically the 2nd account (index 1)
                                                    if compiled.accounts.len() > 1 { Some(1) } else { None }
                                                }
                                                _ => None,
                                            };
                                            
                                            if let Some(account_idx) = mint_account_index {
                                                let mint_idx = compiled.accounts[account_idx] as usize;
                                                if mint_idx < parsed_message.account_keys.len() {
                                                    return Some(parsed_message.account_keys[mint_idx].pubkey.clone());
                                                }
                                            }
                                        }
                                    }
                                },
                                _ => continue,
                            }
                        }
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // For raw messages, check each instruction
                        for instruction in &raw_message.instructions {
                            // Check if this matches our target instruction type
                            if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                if data.len() >= 8 && data[0..8] == discriminator[..] {
                                    // For different instruction types, the mint account is at different positions
                                    let mint_account_index = match instruction_type {
                                        InstructionType::Buy | InstructionType::Sell => {
                                            // For buy/sell, mint is typically the 10th account (index 9)
                                            if instruction.accounts.len() > 9 { Some(9) } else { None }
                                        }
                                        InstructionType::Create => {
                                            // For create, mint is typically the 7th account (index 6)
                                            if instruction.accounts.len() > 6 { Some(6) } else { None }
                                        }
                                        InstructionType::Migrate | InstructionType::MigrateAmm | InstructionType::MigrateCpswap => {
                                            // For migrate, mint is typically the 2nd account (index 1)
                                            if instruction.accounts.len() > 1 { Some(1) } else { None }
                                        }
                                        _ => None,
                                    };
                                    
                                    if let Some(account_idx) = mint_account_index {
                                        let mint_idx = instruction.accounts[account_idx] as usize;
                                        if mint_idx < raw_message.account_keys.len() {
                                            return Some(raw_message.account_keys[mint_idx].clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            _ => return None,
        }
        
        None
    }
    
    fn extract_sol_amount(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<f64> {
        // We need transaction metadata to get balances
        let meta = transaction.meta.as_ref()?;
                
        // Try to find buyer account index
        let mut buyer_index: Option<usize> = None;

        // Extract buyer account index from transaction
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, check each instruction
                        for instruction in &parsed_message.instructions {
                            match instruction {
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    if let Ok(data) = bs58::decode(&compiled.data).into_vec() {
                                        if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                            // Buyer is typically the first account in the accounts list
                                            if !compiled.accounts.is_empty() {
                                                buyer_index = Some(compiled.accounts[0] as usize);
                                                break;
                                            }
                                        }
                                    }
                                },
                                _ => continue,
                            }
                        }
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // For raw messages, check each instruction
                        for instruction in &raw_message.instructions {
                            if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                    // Buyer is typically the first account in the accounts list
                                    if !instruction.accounts.is_empty() {
                                        buyer_index = Some(instruction.accounts[0] as usize);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            },
            _ => {}
        };

        // If we found a buyer index, calculate SOL spent
        if let Some(idx) = buyer_index {
            if idx < meta.pre_balances.len() && idx < meta.post_balances.len() {
                let pre_balance = meta.pre_balances[idx];
                let post_balance = meta.post_balances[idx];
                
                // Calculate difference and convert from lamports to SOL
                if pre_balance > post_balance {
                    let lamports_spent = pre_balance - post_balance;
                    let sol_spent = lamports_spent as f64 / 1_000_000_000.0; // Convert lamports to SOL
                    return Some(sol_spent);
                }
            }
        }

        // Fallback to transaction fee if we couldn't determine the spent amount
        Some(meta.fee as f64 / 1_000_000_000.0)
    }
    
    fn extract_bonding_curve_address(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<String> {
        // Process transaction based on its encoding type
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For buy transactions, the bonding curve is typically at index 2 or 3
                        // We need to check the instruction to find the correct account index
                        for instruction in &parsed_message.instructions {
                            if let solana_transaction_status::UiInstruction::Compiled(compiled) = instruction {
                                let program_idx = compiled.program_id_index as usize;
                                if program_idx >= parsed_message.account_keys.len() {
                                    continue;
                                }

                                let program_id = &parsed_message.account_keys[program_idx].pubkey;
                                if program_id != RAYDIUM_LAUNCHLAB_PROGRAM_ID {
                                    continue;
                                }

                                // Check if this is a buy instruction
                                if let Ok(decoded_data) = bs58::decode(&compiled.data).into_vec() {
                                    if decoded_data.len() >= 8 && (decoded_data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || decoded_data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy/sell instructions, bonding curve is typically at account index 4
                                        if compiled.accounts.len() > 4 {
                                            let bonding_curve_index = compiled.accounts[4] as usize;
                                            if bonding_curve_index < parsed_message.account_keys.len() {
                                                return Some(parsed_message.account_keys[bonding_curve_index].pubkey.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        None
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Handle raw instructions
                        for instruction in &raw_message.instructions {
                            let program_id = if instruction.program_id_index < raw_message.account_keys.len() as u8 {
                                &raw_message.account_keys[instruction.program_id_index as usize]
                            } else {
                                continue;
                            };
                            
                            if program_id == RAYDIUM_LAUNCHLAB_PROGRAM_ID {
                                // Check if this is a buy instruction
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    if data.len() >= 8 && (data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy instructions, bonding curve is typically at account index 4
                                        if instruction.accounts.len() > 4 {
                                            let bonding_curve_index = instruction.accounts[4] as usize;
                                            if bonding_curve_index < raw_message.account_keys.len() {
                                                return Some(raw_message.account_keys[bonding_curve_index].clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        None
                    }
                }
            }
            _ => None,
        }
    }
    
    fn parse_bonding_curve_state(&self, account_data: &mut &[u8]) -> Result<Box<dyn BondingCurveStateTrait>> {
        let state = RaydiumLaunchlabBondingCurveState::deserialize(account_data)
            .map_err(|e| anyhow!("Failed to deserialize bonding curve state: {}", e))?;

        Ok(Box::new(state))
    }

    /// Returns the name of the bonding curve provider
    fn name(&self) -> &'static str {
        "Raydium Launchlab"
    }
}
