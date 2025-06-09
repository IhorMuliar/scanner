use anyhow::{anyhow, Result};
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use std::fmt;

use crate::platform_trait::{BondingCurveStateTrait, InstructionType, TokenPlatformTrait};

/// Constants for Dynamic Bonding Curve program
const DYNAMIC_BONDING_CURVE_PROGRAM_ID: &str = "dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN";
/// Instruction discriminators for Dynamic Bonding Curve program instructions
const SWAP_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [140, 85, 215, 176, 102, 54, 104, 79];
const INITIALIZE_VIRTUAL_POOL_WITH_TOKEN2022_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [169, 118, 51, 78, 145, 110, 220, 155];
const MIGRATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [156, 169, 230, 103, 53, 228, 80, 64];
/// Bonding curve constants for graduation calculations
/// Migration quote threshold - when pool completes and migrates (configurable per pool)
const DEFAULT_MIGRATION_QUOTE_THRESHOLD: f64 = 85.0;
/// Graduation progress thresholds for alerting
const GRADUATION_ALERT_THRESHOLDS: &[f64] = &[50.0, 70.0, 80.0, 90.0, 95.0, 99.0];

/// Volatility tracker for Dynamic Bonding Curve
#[derive(Debug, BorshDeserialize)]
pub struct VolatilityTracker {
    last_update_timestamp: u64,
    padding: [u8; 8],
    sqrt_price_reference: u128,
    volatility_accumulator: u128,
    volatility_reference: u128,
}

/// Pool metrics for Dynamic Bonding Curve
#[derive(Debug, BorshDeserialize)]
pub struct PoolMetrics {
    total_protocol_base_fee: u64,
    total_protocol_quote_fee: u64,
    total_trading_base_fee: u64,
    total_trading_quote_fee: u64,
}

/// Virtual Pool state for Dynamic Bonding Curve tokens
#[derive(Debug, BorshDeserialize)]
pub struct DynamicBondingCurveState {
    /// Volatility tracker
    volatility_tracker: VolatilityTracker,
    /// Config key
    config: Pubkey,
    /// Creator of the pool
    creator: Pubkey,
    /// Base mint (token being traded)
    base_mint: Pubkey,
    /// Base vault
    base_vault: Pubkey,
    /// Quote vault (SOL/USDC vault)
    quote_vault: Pubkey,
    /// Base token reserves
    base_reserve: u64,
    /// Quote token reserves (SOL/USDC)
    quote_reserve: u64,
    /// Protocol base fee accumulated
    protocol_base_fee: u64,
    /// Protocol quote fee accumulated
    protocol_quote_fee: u64,
    /// Partner base fee accumulated
    partner_base_fee: u64,
    /// Partner quote fee accumulated
    partner_quote_fee: u64,
    /// Current sqrt price
    sqrt_price: u128,
    /// Activation point (timestamp/slot)
    activation_point: u64,
    /// Pool type (SPL token or Token2022)
    pool_type: u8,
    /// Migration status flag
    is_migrated: u8,
    /// Partner surplus withdrawal status
    is_partner_withdraw_surplus: u8,
    /// Protocol surplus withdrawal status
    is_protocol_withdraw_surplus: u8,
    /// Migration progress percentage
    migration_progress: u8,
    /// Leftover withdrawal status
    is_withdraw_leftover: u8,
    /// Creator surplus withdrawal status
    is_creator_withdraw_surplus: u8,
    /// Migration fee withdrawal status
    migration_fee_withdraw_status: u8,
    /// Pool metrics
    metrics: PoolMetrics,
    /// Finish curve timestamp
    finish_curve_timestamp: u64,
    /// Creator base fee
    creator_base_fee: u64,
    /// Creator quote fee
    creator_quote_fee: u64,
    /// Padding for future use
    _padding_1: [u64; 7],
}

impl BondingCurveStateTrait for DynamicBondingCurveState {
    /// Calculate the graduation progress percentage (0-100)
    /// Based on migration progress stored in the pool state
    fn calculate_graduation_progress(&self) -> f64 {
        self.migration_progress as f64
    }
    
    /// Calculate current market cap using sqrt price and token supply
    /// Market cap = (sqrt_price^2 / 2^64) * circulating_supply
    fn calculate_market_cap(&self) -> f64 {
        if self.sqrt_price == 0 {
            return 0.0;
        }
        
        // Convert sqrt price to actual price (price = (sqrt_price / 2^64)^2)
        let price_factor = (self.sqrt_price as f64) / (2_u128.pow(64) as f64);
        let price_per_token = price_factor * price_factor;
        
        // For market cap calculation, we use quote reserve as a proxy for circulating value
        // This is a simplified calculation - actual market cap would need total supply info
        self.quote_reserve as f64 / 1_000_000_000.0 // Convert from lamports to SOL
    }
    
    /// Calculate SOL needed to reach graduation threshold
    /// This would typically be read from config, using default for now
    fn needed_for_graduation(&self) -> f64 {
        (DEFAULT_MIGRATION_QUOTE_THRESHOLD - self.current_sol_amount()).max(0.0)
    }
    
    /// Check if token is close to graduation (above any threshold)
    fn is_close_to_graduation(&self) -> Option<f64> {
        let progress = self.calculate_graduation_progress();
        GRADUATION_ALERT_THRESHOLDS.iter()
            .find(|&&threshold| progress >= threshold)
            .copied()
    }

    /// Get current SOL amount in the pool
    fn current_sol_amount(&self) -> f64 {
        self.quote_reserve as f64 / 1_000_000_000.0
    }

    /// Check if the pool has completed migration
    fn is_complete(&self) -> bool {
        self.is_migrated != 0
    }
}

impl fmt::Display for DynamicBondingCurveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DynamicBondingCurve {{ progress: {:.2}%, cap: {:.2} SOL, needed: {:.2} SOL }}",
            self.calculate_graduation_progress(),
            self.calculate_market_cap(),
            self.needed_for_graduation()
        )
    }
}

/// Strategy implementation for Pump.fun platform
pub struct DynamicBondingCurveStrategy;

impl DynamicBondingCurveStrategy {
    pub fn new() -> Self {
        DynamicBondingCurveStrategy
    }
}

impl TokenPlatformTrait for DynamicBondingCurveStrategy {
    fn program_id(&self) -> &'static str {
        DYNAMIC_BONDING_CURVE_PROGRAM_ID
    }
    
    fn buy_instruction_discriminator(&self) -> &'static [u8; 8] {
        &SWAP_INSTRUCTION_DISCRIMINATOR
    }
    
    fn sell_instruction_discriminator(&self) -> &'static [u8; 8] {
        &SWAP_INSTRUCTION_DISCRIMINATOR
    }
    
    fn create_instruction_discriminator(&self) -> &'static [u8; 8] {
        &CREATE_INSTRUCTION_DISCRIMINATOR
    }
    
    fn migrate_instruction_discriminator(&self) -> &'static [u8; 8] {
        &MIGRATE_INSTRUCTION_DISCRIMINATOR
    }
    
    fn is_platform_transaction(&self, transaction: &EncodedTransactionWithStatusMeta) -> bool {
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // Check if Dynamic Bonding Curve program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == DYNAMIC_BONDING_CURVE_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Dynamic Bonding Curve program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == DYNAMIC_BONDING_CURVE_PROGRAM_ID)
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
                                    if program_id != DYNAMIC_BONDING_CURVE_PROGRAM_ID {
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
                            
                            if program_id == DYNAMIC_BONDING_CURVE_PROGRAM_ID {
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
        } else if self.has_instruction_type(transaction, &INITIALIZE_VIRTUAL_POOL_WITH_TOKEN2022_INSTRUCTION_DISCRIMINATOR) {
            Some(InstructionType::InitializeVirtualPoolWithToken2022)
        } else if self.has_instruction_type(transaction, self.migrate_instruction_discriminator()) {
            Some(InstructionType::Migrate)
        } else {
            Some(InstructionType::Other)
        }
    }
    
    fn extract_mint_address(&self, transaction: &EncodedTransactionWithStatusMeta, instruction_type: &InstructionType) -> Option<String> {
        // Get the appropriate discriminator for the instruction type
        let discriminator = match instruction_type {
            InstructionType::Swap => &SWAP_INSTRUCTION_DISCRIMINATOR,
            InstructionType::Create => &CREATE_INSTRUCTION_DISCRIMINATOR,
            InstructionType::InitializeVirtualPoolWithToken2022 => &INITIALIZE_VIRTUAL_POOL_WITH_TOKEN2022_INSTRUCTION_DISCRIMINATOR,
            InstructionType::Migrate => &MIGRATE_INSTRUCTION_DISCRIMINATOR,
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
                                            let mint_account_index =                             match instruction_type {
                                InstructionType::Swap => {
                                    // For swap, base mint is at index 7 in accounts
                                    if compiled.accounts.len() > 7 { Some(7) } else { None }
                                }
                                InstructionType::Create | InstructionType::InitializeVirtualPoolWithToken2022 => {
                                    // For create, base mint is at index 3 in accounts
                                    if compiled.accounts.len() > 3 { Some(3) } else { None }
                                }
                                InstructionType::Migrate => {
                                    // For migrate, base mint is at index 14 in accounts
                                    if compiled.accounts.len() > 14 { Some(14) } else { None }
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
                                        InstructionType::Swap => {
                                            // For swap, base mint is at index 7 in accounts
                                            if instruction.accounts.len() > 7 { Some(7) } else { None }
                                        }
                                        InstructionType::Create | InstructionType::InitializeVirtualPoolWithToken2022 => {
                                            // For create, base mint is at index 3 in accounts
                                            if instruction.accounts.len() > 3 { Some(3) } else { None }
                                        }
                                        InstructionType::Migrate => {
                                            // For migrate, base mint is at index 14 in accounts
                                            if instruction.accounts.len() > 14 { Some(14) } else { None }
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
                                        if data.len() >= 8 && data[0..8] == SWAP_INSTRUCTION_DISCRIMINATOR {
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
                                if data.len() >= 8 && data[0..8] == SWAP_INSTRUCTION_DISCRIMINATOR {
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
                                if program_id != DYNAMIC_BONDING_CURVE_PROGRAM_ID {
                                    continue;
                                }

                                // Check if this is a swap instruction
                                if let Ok(decoded_data) = bs58::decode(&compiled.data).into_vec() {
                                    if decoded_data.len() >= 8 && decoded_data[0..8] == SWAP_INSTRUCTION_DISCRIMINATOR {
                                        // For swap instructions, pool (bonding curve) is at account index 2
                                        if compiled.accounts.len() > 2 {
                                            let bonding_curve_index = compiled.accounts[2] as usize;
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
                            
                            if program_id == DYNAMIC_BONDING_CURVE_PROGRAM_ID {
                                // Check if this is a swap instruction
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    if data.len() >= 8 && data[0..8] == SWAP_INSTRUCTION_DISCRIMINATOR {
                                        // For swap instructions, pool (bonding curve) is at account index 2
                                        if instruction.accounts.len() > 2 {
                                            let bonding_curve_index = instruction.accounts[2] as usize;
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
        let state = DynamicBondingCurveState::deserialize(account_data)
            .map_err(|e| anyhow!("Failed to deserialize bonding curve state: {}", e))?;

        Ok(Box::new(state))
    }

    /// Returns the name of the bonding curve provider
    fn name(&self) -> &'static str {
        "Dynamic Bonding Curve"
    }
}
