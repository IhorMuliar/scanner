use anyhow::Result;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use std::fmt::Debug;

/// Enum representing the different types of instructions that can be processed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionType {
    Buy,
    Sell,
    Create,
    /// General migrate instruction (for platforms with single migrate type)
    Migrate,
    /// AMM-specific migrate instruction (for Raydium Launchlab)
    MigrateAmm,
    /// CPSWAP-specific migrate instruction (for Raydium Launchlab)
    MigrateCpswap,
    Other,
}

/// Trait defining platform-specific functionality
pub trait TokenPlatformTrait: Send + Sync + 'static {
    /// Get the program ID for this platform
    fn program_id(&self) -> &'static str;
    
    /// Get instruction discriminators for this platform
    fn buy_instruction_discriminator(&self) -> &'static [u8; 8];
    fn sell_instruction_discriminator(&self) -> &'static [u8; 8];
    fn create_instruction_discriminator(&self) -> &'static [u8; 8];
    /// General migrate instruction discriminator (for platforms with single migrate type)
    fn migrate_instruction_discriminator(&self) -> &'static [u8; 8];
    
    /// AMM-specific migrate instruction discriminator (optional, for platforms like Raydium Launchlab)
    fn migrate_amm_instruction_discriminator(&self) -> Option<&'static [u8; 8]> {
        None
    }
    
    /// CPSWAP-specific migrate instruction discriminator (optional, for platforms like Raydium Launchlab)
    fn migrate_cpswap_instruction_discriminator(&self) -> Option<&'static [u8; 8]> {
        None
    }
    
    /// Determine if a transaction involves this platform
    fn is_platform_transaction(&self, transaction: &EncodedTransactionWithStatusMeta) -> bool;
    
    /// Check if a transaction contains a specific instruction type
    fn has_instruction_type(&self, transaction: &EncodedTransactionWithStatusMeta, discriminator: &[u8; 8]) -> bool;
    
    /// Determine instruction type for a transaction
    fn determine_instruction_type(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<InstructionType>;
    
    /// Extract mint address from a transaction
    fn extract_mint_address(&self, transaction: &EncodedTransactionWithStatusMeta, instruction_type: &InstructionType) -> Option<String>;
    
    /// Extract SOL amount from a buy transaction
    fn extract_sol_amount(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<f64>;
    
    /// Extract bonding curve address from a transaction
    fn extract_bonding_curve_address(&self, transaction: &EncodedTransactionWithStatusMeta) -> Option<String>;
    
    /// Parse bonding curve state from account data
    fn parse_bonding_curve_state(&self, account_data: &mut &[u8]) -> Result<Box<dyn BondingCurveStateTrait>>;
    
    /// Get platform name for logging
    fn name(&self) -> &'static str;
}

/// Trait for bonding curve state across platforms
pub trait BondingCurveStateTrait: Send + Sync + Debug {
    /// Calculate graduation progress percentage (0-100)
    fn calculate_graduation_progress(&self) -> f64;
    
    /// Calculate current market cap in quote token
    fn calculate_market_cap(&self) -> f64;
    
    /// Calculate quote token needed for graduation
    fn needed_for_graduation(&self) -> f64;
    
    /// Check if token is close to graduation (above any threshold)
    fn is_close_to_graduation(&self) -> Option<f64>;
    
    /// Get current quote token amount
    fn current_sol_amount(&self) -> f64;
    
    /// Check if curve is complete
    fn is_complete(&self) -> bool;
} 