use solana_sdk::pubkey::Pubkey;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use log::warn;

/// Known program IDs for platforms we're monitoring
#[derive(Debug, Clone)]
pub struct ProgramIds {
    pub pump_fun: Pubkey,
    pub raydium_amm: Pubkey,
    pub meteora: Pubkey,
}

impl Default for ProgramIds {
    fn default() -> Self {
        Self {
            // Note: These are placeholder program IDs for demonstration purposes
            // In production, you would use the actual program IDs
            pump_fun: Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap(),
            raydium_amm: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap(),
            meteora: Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap(),
        }
    }
}

impl ProgramIds {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn get_all(&self) -> Vec<Pubkey> {
        vec![self.pump_fun, self.raydium_amm, self.meteora]
    }
}

/// Platform enum for different DEX platforms
#[derive(Debug, Clone, PartialEq)]
pub enum Platform {
    PumpFun,
    Raydium,
    Meteora,
    Unknown,
}

impl Platform {
    pub fn from_program_id(program_id: &Pubkey) -> Self {
        let program_ids = ProgramIds::new();
        
        if program_id == &program_ids.pump_fun {
            Platform::PumpFun
        } else if program_id == &program_ids.raydium_amm {
            Platform::Raydium
        } else if program_id == &program_ids.meteora {
            Platform::Meteora
        } else {
            Platform::Unknown
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::PumpFun => "Pump.fun",
            Platform::Raydium => "Raydium",
            Platform::Meteora => "Meteora",
            Platform::Unknown => "Unknown",
        }
    }
}

/// Token information extracted from transactions
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token_mint: Pubkey,
    pub volume: u64,
    pub buyer: Pubkey,
    pub platform: Platform,
}

/// Token metrics tracked in memory
#[derive(Debug, Clone)]
pub struct TokenMetrics {
    pub mint: Pubkey,
    pub symbol: String,
    pub total_volume: u64,
    pub unique_buyers: HashSet<String>,
    pub first_seen: u64,
    pub last_seen: u64,
    pub platform: Platform,
    pub transactions: u64,
}

impl TokenMetrics {
    pub fn new(token_info: TokenInfo, timestamp: u64) -> Self {
        let mut unique_buyers = HashSet::new();
        unique_buyers.insert(token_info.buyer.to_string());
        
        Self {
            mint: token_info.token_mint,
            symbol: generate_symbol(&token_info.token_mint),
            total_volume: token_info.volume,
            unique_buyers,
            first_seen: timestamp,
            last_seen: timestamp,
            platform: token_info.platform,
            transactions: 1,
        }
    }
    
    pub fn update(&mut self, token_info: TokenInfo, timestamp: u64) {
        self.total_volume += token_info.volume;
        self.unique_buyers.insert(token_info.buyer.to_string());
        self.last_seen = timestamp;
        self.transactions += 1;
    }
    
    pub fn volume_in_sol(&self) -> f64 {
        self.total_volume as f64 / 1e9
    }
    
    pub fn unique_buyers_count(&self) -> usize {
        self.unique_buyers.len()
    }
    
    pub fn age_minutes(&self, current_time: u64) -> u64 {
        let diff_ms = current_time - self.first_seen;

        if diff_ms == 0 {
            return 0;
        } else if diff_ms < 60 * 1000 {
            return 1;
        }
        
        diff_ms / (60 * 1000)
    }
}

/// Hot token information for display
#[derive(Debug, Clone)]
pub struct HotToken {
    pub symbol: String,
    pub volume_in_sol: f64,
    pub unique_buyers_count: usize,
    pub age_minutes: u64,
    pub platform: Platform,
    pub mint: Pubkey,
}

impl From<&TokenMetrics> for HotToken {
    fn from(metrics: &TokenMetrics) -> Self {
        let current_time = current_timestamp();
        
        Self {
            symbol: metrics.symbol.clone(),
            volume_in_sol: metrics.volume_in_sol(),
            unique_buyers_count: metrics.unique_buyers_count(),
            age_minutes: metrics.age_minutes(current_time),
            platform: metrics.platform.clone(),
            mint: metrics.mint,
        }
    }
}

/// Formats lamports to SOL with specified decimal places
pub const fn format_sol(lamports: u64) -> f64 {
    lamports as f64 / 1e9
}

/// Calculates token age in minutes
pub const fn get_token_age(first_seen: u64, current_time: u64) -> u64 {
    (current_time - first_seen) / (60 * 1000)
}

/// Generates a simulated token symbol from the mint address
/// In a production app, you would query token metadata
pub fn generate_symbol(token_mint: &Pubkey) -> String {
    format!("${}", &token_mint.to_string()[..4])
}

/// Safely validates if string is a valid Solana public key
pub fn is_valid_public_key(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}

/// Gets current timestamp in milliseconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Converts block time to milliseconds timestamp
pub fn block_time_to_timestamp(block_time: Option<i64>) -> u64 {
    match block_time {
        Some(time) => (time * 1000) as u64,
        None => current_timestamp(),
    }
}

/// Sleep utility for async functions
pub async fn sleep(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
} 