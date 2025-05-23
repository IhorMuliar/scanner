use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashSet;

/// Represents a trading platform
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    PumpFun,
    Raydium,
    Meteora,
    Unknown,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::PumpFun => write!(f, "Pump.fun"),
            Platform::Raydium => write!(f, "Raydium"),
            Platform::Meteora => write!(f, "Meteora"),
            Platform::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Token information extracted from a transaction
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token_mint: Pubkey,
    pub volume: u64, // in lamports
    pub buyer: Pubkey,
    pub platform: Platform,
}

/// Comprehensive metrics for tracking token activity
#[derive(Debug, Clone)]
pub struct TokenMetrics {
    pub mint: Pubkey,
    pub symbol: String,
    pub total_volume: u64, // in lamports
    pub unique_buyers: HashSet<Pubkey>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub platform: Platform,
    pub transaction_count: u64,
}

impl TokenMetrics {
    pub fn new(token_info: TokenInfo, timestamp: DateTime<Utc>) -> Self {
        let mut unique_buyers = HashSet::new();
        unique_buyers.insert(token_info.buyer);

        Self {
            mint: token_info.token_mint,
            symbol: generate_symbol(&token_info.token_mint),
            total_volume: token_info.volume,
            unique_buyers,
            first_seen: timestamp,
            last_seen: timestamp,
            platform: token_info.platform,
            transaction_count: 1,
        }
    }

    pub fn update(&mut self, token_info: TokenInfo, timestamp: DateTime<Utc>) {
        self.total_volume += token_info.volume;
        self.unique_buyers.insert(token_info.buyer);
        self.last_seen = timestamp;
        self.transaction_count += 1;
    }

    pub fn volume_in_sol(&self) -> f64 {
        self.total_volume as f64 / 1_000_000_000.0
    }

    pub fn unique_buyers_count(&self) -> usize {
        self.unique_buyers.len()
    }

    pub fn age_minutes(&self) -> i64 {
        let now = Utc::now();
        (now - self.first_seen).num_minutes()
    }
}

/// Hot token information for console output
#[derive(Debug, Clone)]
pub struct HotToken {
    pub symbol: String,
    pub volume_sol: f64,
    pub unique_buyers_count: usize,
    pub age_minutes: i64,
    pub platform: Platform,
}

impl From<&TokenMetrics> for HotToken {
    fn from(metrics: &TokenMetrics) -> Self {
        Self {
            symbol: metrics.symbol.clone(),
            volume_sol: metrics.volume_in_sol(),
            unique_buyers_count: metrics.unique_buyers_count(),
            age_minutes: metrics.age_minutes(),
            platform: metrics.platform.clone(),
        }
    }
}

/// Known program IDs for the platforms we're monitoring
pub struct ProgramIds;

impl ProgramIds {
    // Note: These are placeholder program IDs for demonstration purposes
    // In production, you would use the actual program IDs
    pub const PUMP_FUN: &'static str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
    pub const RAYDIUM_AMM: &'static str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
    pub const METEORA: &'static str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

    pub fn get_all() -> Vec<Pubkey> {
        vec![
            Self::PUMP_FUN.parse().unwrap(),
            Self::RAYDIUM_AMM.parse().unwrap(),
            Self::METEORA.parse().unwrap(),
        ]
    }

    pub fn determine_platform(program_id: &Pubkey) -> Platform {
        let program_str = program_id.to_string();
        match program_str.as_str() {
            Self::PUMP_FUN => Platform::PumpFun,
            Self::RAYDIUM_AMM => Platform::Raydium,
            Self::METEORA => Platform::Meteora,
            _ => Platform::Unknown,
        }
    }
}

/// Generates a token symbol from the mint address
/// In production, you would query token metadata from the blockchain
fn generate_symbol(token_mint: &Pubkey) -> String {
    let mint_str = token_mint.to_string();
    format!("${}", &mint_str[0..4])
} 