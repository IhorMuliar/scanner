use anyhow::{anyhow, Result};
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use std::fmt;

use crate::platform_trait::{BondingCurveStateTrait, InstructionType, TokenPlatformTrait};

/// Constants for Boop.fun program
const BOOP_FUN_PROGRAM_ID: &str = "boop8hVGQGqehUK2iVEMEnMrL5RbjywRzHKBmBE7ry4";
/// Instruction discriminators for Boop.fun program instructions
const BUY_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [138, 127, 14, 91, 38, 87, 115, 105];
const SELL_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [109, 61, 40, 187, 230, 176, 135, 174];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [84, 52, 204, 228, 24, 140, 234, 75];
const MIGRATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [45, 235, 225, 181, 17, 218, 64, 130];
/// Graduation progress thresholds for alerting
const GRADUATION_ALERT_THRESHOLDS: &[f64] = &[50.0, 70.0, 80.0, 90.0, 95.0, 99.0];

/// Bonding curve status enum matching the Boop.fun IDL
#[derive(Debug, BorshDeserialize, PartialEq)]
pub enum BondingCurveStatus {
    Trading,
    Graduated,
    PoolPriceCorrected,
    LiquidityProvisioned,
    LiquidityLocked,
}

/// Bonding curve state for Boop.fun tokens matching the actual IDL structure
#[derive(Debug, BorshDeserialize)]
pub struct BoopFunBondingCurveState {
    /// Creator of the bonding curve
    creator: Pubkey,
    /// Token mint address
    mint: Pubkey,
    /// Virtual SOL reserves (for pricing calculations)
    virtual_sol_reserves: u64,
    /// Virtual token reserves (for pricing calculations)
    virtual_token_reserves: u64,
    /// SOL target amount for graduation to Raydium
    graduation_target: u64,
    /// Fee taken during graduation process
    graduation_fee: u64,
    /// Real SOL reserves (actual SOL collected)
    sol_reserves: u64,
    /// Real token reserves (actual tokens remaining)
    token_reserves: u64,
    /// Damping term affecting curve calculations
    damping_term: u8,
    /// Swap fee in basis points (e.g., 100 = 1%)
    swap_fee_basis_points: u8,
    /// Token percentage for stakers in basis points
    token_for_stakers_basis_points: u16,
    /// Current status of the bonding curve
    status: BondingCurveStatus,
}

impl BondingCurveStateTrait for BoopFunBondingCurveState {
    /// Calculate the graduation progress percentage (0-100)
    /// Based on: (sol_reserves * 100) / graduation_target
    /// Boop.fun graduation is based on SOL collected, not tokens sold
    fn calculate_graduation_progress(&self) -> f64 {
        if self.graduation_target == 0 {
            return 0.0;
        }
        
        // Progress is based on actual SOL collected vs graduation target
        (self.sol_reserves as f64 * 100.0) / self.graduation_target as f64
    }
    
    /// Calculate current market cap in SOL using bonding curve formula
    /// Using virtual reserves for accurate pricing: virtual_sol_reserves / virtual_token_reserves
    fn calculate_market_cap(&self) -> f64 {
        if self.virtual_token_reserves == 0 {
            return 0.0;
        }
        
        // Current price per token in SOL using virtual reserves
        let price_per_token = self.virtual_sol_reserves as f64 / self.virtual_token_reserves as f64;
        
        // Calculate circulating supply as tokens that have been purchased
        // This is an approximation based on the difference between virtual and real reserves
        let tokens_in_circulation = if self.virtual_token_reserves > self.token_reserves {
            self.virtual_token_reserves - self.token_reserves
        } else {
            0
        };

        // Market cap = price * circulating supply
        (price_per_token * tokens_in_circulation as f64) / 1_000_000_000.0 // Convert from lamports
    }
    
    /// Calculate SOL needed to reach graduation threshold
    /// Uses the dynamic graduation_target from the bonding curve account
    fn needed_for_graduation(&self) -> f64 {
        let graduation_target_sol = self.graduation_target as f64 / 1_000_000_000.0; // Convert to SOL
        let current_sol = self.current_sol_amount();
        (graduation_target_sol - current_sol).max(0.0)
    }
    
    /// Check if token is close to graduation (above any threshold)
    fn is_close_to_graduation(&self) -> Option<f64> {
        let progress = self.calculate_graduation_progress();
        GRADUATION_ALERT_THRESHOLDS.iter()
            .find(|&&threshold| progress >= threshold)
            .copied()
    }

    /// Get current SOL amount collected in the bonding curve
    fn current_sol_amount(&self) -> f64 {
        self.sol_reserves as f64 / 1_000_000_000.0
    }

    /// Check if the bonding curve has completed (graduated)
    fn is_complete(&self) -> bool {
        matches!(self.status, BondingCurveStatus::Graduated | 
                            BondingCurveStatus::PoolPriceCorrected | 
                            BondingCurveStatus::LiquidityProvisioned | 
                            BondingCurveStatus::LiquidityLocked)
    }
}

impl fmt::Display for BoopFunBondingCurveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BoopFunBondingCurve {{ progress: {:.2}%, cap: {:.2} SOL, needed: {:.2} SOL, status: {:?} }}",
            self.calculate_graduation_progress(),
            self.calculate_market_cap(),
            self.needed_for_graduation(),
            self.status
        )
    }
}

/// Strategy implementation for Pump.fun platform
pub struct BoopFunStrategy;

impl BoopFunStrategy {
    pub fn new() -> Self {
        BoopFunStrategy
    }
}

impl TokenPlatformTrait for BoopFunStrategy {
    fn program_id(&self) -> &'static str {
        BOOP_FUN_PROGRAM_ID
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
        &MIGRATE_INSTRUCTION_DISCRIMINATOR
    }
    
    fn is_platform_transaction(&self, transaction: &EncodedTransactionWithStatusMeta) -> bool {
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // Check if Boop.fun program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == BOOP_FUN_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Boop.fun program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == BOOP_FUN_PROGRAM_ID)
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
                                    if program_id != BOOP_FUN_PROGRAM_ID {
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
                            
                            if program_id == BOOP_FUN_PROGRAM_ID {
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
        } else if self.has_instruction_type(transaction, self.migrate_instruction_discriminator()) {
            Some(InstructionType::Migrate)
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
                                            let mint_account_index = match instruction_type {
                                                InstructionType::Buy | InstructionType::Sell | InstructionType::Migrate => {
                                                    // For buy/sell/migrate, mint is typically the 1st account (index 0)
                                                    if compiled.accounts.len() > 0 { Some(0) } else { None }
                                                }
                                                InstructionType::Create => {
                                                    // For create, mint is typically the 3rd account (index 2)
                                                    if compiled.accounts.len() > 2 { Some(2) } else { None }
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
                                        InstructionType::Buy | InstructionType::Sell | InstructionType::Migrate => {
                                            // For buy/sell/migrate, mint is typically the 1st account (index 0)
                                            if instruction.accounts.len() > 0 { Some(0) } else { None }
                                        }
                                        InstructionType::Create => {
                                            // For create, mint is typically the 3rd account (index 2)
                                            if instruction.accounts.len() > 2 { Some(2) } else { None }
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
                                if program_id != BOOP_FUN_PROGRAM_ID {
                                    continue;
                                }

                                // Check if this is a buy instruction
                                if let Ok(decoded_data) = bs58::decode(&compiled.data).into_vec() {
                                    if decoded_data.len() >= 8 && (decoded_data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || decoded_data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy/sell instructions, bonding curve is typically at account index 3
                                        if compiled.accounts.len() > 3 {
                                            let bonding_curve_index = compiled.accounts[3] as usize;
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
                            
                            if program_id == BOOP_FUN_PROGRAM_ID {
                                // Check if this is a buy instruction
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                        // For buy instructions, bonding curve is typically at account index 3
                                        if instruction.accounts.len() > 3 {
                                            let bonding_curve_index = instruction.accounts[3] as usize;
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
        let state = BoopFunBondingCurveState::deserialize(account_data)
            .map_err(|e| anyhow!("Failed to deserialize bonding curve state: {}", e))?;

        Ok(Box::new(state))
    }

    /// Returns the name of the bonding curve provider
    fn name(&self) -> &'static str {
        "Boop.fun"
    }
}
