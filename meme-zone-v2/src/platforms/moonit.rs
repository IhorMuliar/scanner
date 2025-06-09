use anyhow::{anyhow, Result};
use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use std::fmt;

use crate::platform_trait::{BondingCurveStateTrait, InstructionType, TokenPlatformTrait};

/// Constants for Moonit program
const MOONIT_PROGRAM_ID: &str = "MoonCVVNZFSYkqNXP6bxHLPL6QQJiMagDL3qcqUQTrG";
/// Instruction discriminators for Moonit program instructions
const BUY_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const SELL_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [3, 44, 164, 184, 123, 13, 245, 179];
const MIGRATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [42, 229, 10, 231, 189, 62, 193, 174];

/// Currency enum as defined in the IDL
#[derive(Debug, BorshDeserialize, Clone, Copy)]
pub enum Currency {
    Sol,
}

/// Curve type enum as defined in the IDL
#[derive(Debug, BorshDeserialize, Clone, Copy)]
pub enum CurveType {
    LinearV1,
    ConstantProductV1,
}

/// Migration target enum as defined in the IDL
#[derive(Debug, BorshDeserialize, Clone, Copy)]
pub enum MigrationTarget {
    Raydium,
    Meteora,
}

/// Bonding curve state for Moonit tokens (matches IDL CurveAccount structure)
#[derive(Debug, BorshDeserialize)]
pub struct MoonitBondingCurveState {
    /// Total supply of the token
    pub total_supply: u64,
    /// Current amount of collateral (SOL) in the curve
    pub curve_amount: u64,
    /// Token mint address
    pub mint: Pubkey,
    /// Token decimals
    pub decimals: u8,
    /// Collateral currency type (SOL)
    pub collateral_currency: Currency,
    /// Type of bonding curve (Linear or Constant Product)
    pub curve_type: CurveType,
    /// Market cap threshold for migration (in lamports)
    pub marketcap_threshold: u64,
    /// Currency for market cap threshold
    pub marketcap_currency: Currency,
    /// Migration fee (in lamports)
    pub migration_fee: u64,
    /// Coefficient B for curve calculations
    pub coef_b: u32,
    /// Bump seed for PDA
    pub bump: u8,
    /// Target DEX for migration
    pub migration_target: MigrationTarget,
}

impl BondingCurveStateTrait for MoonitBondingCurveState {
    /// Calculate the graduation progress percentage (0-100)
    /// For Moonit, progress is based on SOL collected vs threshold
    fn calculate_graduation_progress(&self) -> f64 {
        if self.marketcap_threshold == 0 {
            return 0.0;
        }
        
        // Progress based on current SOL amount vs threshold
        let current_sol_lamports = self.curve_amount;
        let threshold_lamports = self.marketcap_threshold;
        
        if current_sol_lamports >= threshold_lamports {
            return 100.0;
        }
        
        (current_sol_lamports as f64 / threshold_lamports as f64) * 100.0
    }
    
    /// Calculate current market cap in SOL
    /// For Moonit, market cap calculation depends on curve type
    fn calculate_market_cap(&self) -> f64 {
        match self.curve_type {
            CurveType::LinearV1 => {
                // For linear curves, market cap grows linearly with SOL collected
                // This is a simplified calculation - actual implementation may differ
                self.curve_amount as f64 / 1_000_000_000.0
            }
            CurveType::ConstantProductV1 => {
                // For constant product curves, use more complex calculation
                // This would require the actual curve parameters (k constant, etc.)
                // For now, using SOL amount as approximation
                self.curve_amount as f64 / 1_000_000_000.0
            }
        }
    }
    
    /// Calculate SOL needed to reach graduation threshold
    fn needed_for_graduation(&self) -> f64 {
        if self.curve_amount >= self.marketcap_threshold {
            return 0.0;
        }
        
        let needed_lamports = self.marketcap_threshold - self.curve_amount;
        needed_lamports as f64 / 1_000_000_000.0
    }
    
    /// Check if token is close to graduation (above certain thresholds)
    fn is_close_to_graduation(&self) -> Option<f64> {
        let progress = self.calculate_graduation_progress();
        
        // Define graduation alert thresholds
        const GRADUATION_ALERT_THRESHOLDS: &[f64] = &[50.0, 70.0, 80.0, 90.0, 95.0, 99.0];
        
        GRADUATION_ALERT_THRESHOLDS.iter()
            .find(|&&threshold| progress >= threshold)
            .copied()
    }

    /// Get current SOL amount in the curve
    fn current_sol_amount(&self) -> f64 {
        self.curve_amount as f64 / 1_000_000_000.0
    }

    /// Check if the curve has completed (reached threshold)
    fn is_complete(&self) -> bool {
        self.curve_amount >= self.marketcap_threshold
    }
}

impl fmt::Display for MoonitBondingCurveState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MoonitBondingCurve {{ progress: {:.2}%, cap: {:.2} SOL, needed: {:.2} SOL, type: {:?}, target: {:?} }}",
            self.calculate_graduation_progress(),
            self.calculate_market_cap(),
            self.needed_for_graduation(),
            self.curve_type,
            self.migration_target
        )
    }
}

/// Strategy implementation for Moonit platform
pub struct MoonitStrategy;

impl MoonitStrategy {
    pub fn new() -> Self {
        MoonitStrategy
    }
}

impl TokenPlatformTrait for MoonitStrategy {
    fn program_id(&self) -> &'static str {
        MOONIT_PROGRAM_ID
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
                        // Check if Pump.fun program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == MOONIT_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Pump.fun program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == MOONIT_PROGRAM_ID)
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
                                    if program_id != MOONIT_PROGRAM_ID {
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
                            
                            if program_id == MOONIT_PROGRAM_ID {
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
                                            // Based on IDL account ordering
                                            let mint_account_index = match instruction_type {
                                                InstructionType::Buy | InstructionType::Sell => {
                                                    // For buy/sell, mint is the 7th account (index 6)
                                                    // Order: sender, senderTokenAccount, curveAccount, curveTokenAccount, dexFee, helioFee, mint
                                                    if compiled.accounts.len() > 6 { Some(6) } else { None }
                                                }
                                                InstructionType::Create => {
                                                    // For tokenMint, mint is the 4th account (index 3)
                                                    // Order: sender, backendAuthority, curveAccount, mint
                                                    if compiled.accounts.len() > 3 { Some(3) } else { None }
                                                }
                                                InstructionType::Migrate => {
                                                    // For migrateFunds, mint is the 6th account (index 5)
                                                    // Order: backendAuthority, migrationAuthority, curveAccount, curveTokenAccount, migrationAuthorityTokenAccount, mint
                                                    if compiled.accounts.len() > 5 { Some(5) } else { None }
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
                                    // Based on IDL account ordering
                                    let mint_account_index = match instruction_type {
                                        InstructionType::Buy | InstructionType::Sell => {
                                            // For buy/sell, mint is the 7th account (index 6)
                                            // Order: sender, senderTokenAccount, curveAccount, curveTokenAccount, dexFee, helioFee, mint
                                            if instruction.accounts.len() > 6 { Some(6) } else { None }
                                        }
                                        InstructionType::Create => {
                                            // For tokenMint, mint is the 4th account (index 3)
                                            // Order: sender, backendAuthority, curveAccount, mint
                                            if instruction.accounts.len() > 3 { Some(3) } else { None }
                                        }
                                        InstructionType::Migrate => {
                                            // For migrateFunds, mint is the 6th account (index 5)
                                            // Order: backendAuthority, migrationAuthority, curveAccount, curveTokenAccount, migrationAuthorityTokenAccount, mint
                                            if instruction.accounts.len() > 5 { Some(5) } else { None }
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
                                if program_id != MOONIT_PROGRAM_ID {
                                    continue;
                                }

                                // Check if this is a buy/sell instruction
                                if let Ok(decoded_data) = bs58::decode(&compiled.data).into_vec() {
                                    if decoded_data.len() >= 8 && (decoded_data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || decoded_data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy/sell instructions, curveAccount is at index 2
                                        // Order: sender, senderTokenAccount, curveAccount, ...
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
                            
                            if program_id == MOONIT_PROGRAM_ID {
                                // Check if this is a buy/sell instruction
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    if data.len() >= 8 && (data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR || data[0..8] == SELL_INSTRUCTION_DISCRIMINATOR) {
                                        // For buy/sell instructions, curveAccount is at index 2
                                        // Order: sender, senderTokenAccount, curveAccount, ...
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
        let state = MoonitBondingCurveState::deserialize(account_data)
            .map_err(|e| anyhow!("Failed to deserialize bonding curve state: {}", e))?;

        Ok(Box::new(state))
    }

    /// Returns the name of the bonding curve provider
    fn name(&self) -> &'static str {
        "Moonit"
    }
}
