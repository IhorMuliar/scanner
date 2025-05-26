use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Formats lamports to SOL with 2 decimal places
pub fn format_sol(lamports: u64) -> String {
    let sol = lamports as f64 / 1e9;
    format!("{:.2} SOL", sol)
}

/// Calculates token age in minutes
pub fn get_token_age(first_seen: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let age_ms = now - first_seen;
    age_ms / (60 * 1000)
}

/// Generates a simulated token symbol from the mint address
/// In a production app, you would query token metadata
pub fn generate_symbol(token_mint: &str) -> String {
    format!("${}", &token_mint[..4])
}

/// Safely validates if string is a valid Solana public key
pub fn is_valid_public_key(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}

/// Known program IDs for platforms we're monitoring
/// Note: These are placeholder program IDs for demonstration purposes
pub struct ProgramIds;

impl ProgramIds {
    // In a production app, you would use the actual program IDs
    pub const PUMP_FUN: &'static str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
    pub const RAYDIUM_AMM: &'static str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    pub const METEORA: &'static str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
}

/// Handles sleep/delay in async functions
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
}

/// Determines platform name from program ID
pub fn determine_platform(program_id: &str) -> String {
    match program_id {
        ProgramIds::PUMP_FUN => "Pump.fun".to_string(),
        ProgramIds::RAYDIUM_AMM => "Raydium".to_string(),
        ProgramIds::METEORA => "Meteora".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Token information extracted from a transaction
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token_mint: String,
    pub volume: u64,
    pub buyer: String,
    pub platform: String,
}

/// Creates token information from transaction data
pub fn create_token_info(token_mint: String, volume: u64, buyer: String, platform: String) -> TokenInfo {
    TokenInfo {
        token_mint,
        volume,
        buyer,
        platform,
    }
} 