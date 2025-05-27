use log::info;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::collections::{HashMap, HashSet};

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
    // format!("${}", &token_mint)
    token_mint.to_string()
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

/// Comprehensive token metrics for tracking trading activity
#[derive(Debug, Clone)]
pub struct TokenMetrics {
    pub token_mint: String,
    pub symbol: String,
    pub total_volume: u64,
    pub unique_buyers: HashSet<String>,
    pub first_seen: u64,
    pub last_activity: u64,
    pub platform: String,
    pub transaction_count: u64,
    pub volume_history: Vec<VolumeEntry>,
}

/// Volume entry for tracking volume over time
#[derive(Debug, Clone)]
pub struct VolumeEntry {
    pub timestamp: u64,
    pub volume: u64,
    pub buyer: String,
}

impl TokenMetrics {
    /// Creates new token metrics
    pub fn new(token_mint: String, platform: String) -> Self {
        let symbol = generate_symbol(&token_mint);
        let now = get_current_timestamp();
        
        Self {
            token_mint,
            symbol,
            total_volume: 0,
            unique_buyers: HashSet::new(),
            first_seen: now,
            last_activity: now,
            platform,
            transaction_count: 0,
            volume_history: Vec::new(),
        }
    }

    /// Updates metrics with new transaction data
    pub fn update(&mut self, volume: u64, buyer: String) {
        self.total_volume += volume;
        self.unique_buyers.insert(buyer.clone());
        self.last_activity = get_current_timestamp();
        self.transaction_count += 1;
        
        self.volume_history.push(VolumeEntry {
            timestamp: self.last_activity,
            volume,
            buyer,
        });
    }

    /// Gets the age of the token in minutes
    pub fn get_age_minutes(&self) -> u64 {
        get_token_age(self.first_seen)
    }

    /// Gets total volume in SOL
    pub fn get_volume_sol(&self) -> f64 {
        self.total_volume as f64 / 1e9
    }

    /// Gets number of unique buyers
    pub fn get_buyer_count(&self) -> usize {
        self.unique_buyers.len()
    }

    /// Gets SOL per buy ratio
    pub fn get_sol_per_buy_ratio(&self) -> f64 {
        if self.transaction_count == 0 {
            0.0
        } else {
            self.get_volume_sol() / self.transaction_count as f64
        }
    }

    /// Calculate recent volume and transaction count within specified time window
    pub fn get_recent_activity(&self, minutes_window: u64) -> (f64, u64) {
        let cutoff_time = get_current_timestamp() - (minutes_window * 60 * 1000);
        
        let recent_entries = self.volume_history.iter()
            .filter(|entry| entry.timestamp >= cutoff_time);
        
        let recent_volume: u64 = recent_entries.clone().map(|entry| entry.volume).sum();
        let recent_tx_count = recent_entries.count() as u64;
        
        (recent_volume as f64 / 1e9, recent_tx_count)
    }
    
    /// Checks if token meets ratio-based spike criteria
    pub fn is_ratio_spike(&self, ratio_threshold: f64, time_window_minutes: u64) -> bool {
        // Get recent activity within the time window
        let (recent_sol, recent_tx_count) = self.get_recent_activity(time_window_minutes);
        
        // Calculate the ratio of SOL to buy transactions
        let ratio = if recent_tx_count == 0 {
            0.0
        } else {
            recent_sol / recent_tx_count as f64
        };
        
        // Token must have at least 5 transactions to be considered for a ratio spike
        recent_tx_count >= 5 && ratio > ratio_threshold
    }

    /// Checks if token meets spike criteria
    pub fn is_spike(&self, volume_threshold: f64, buyers_threshold: usize, age_threshold_minutes: u64) -> bool {
        info!("🔍 Processing token metrics age {:?} and {:?}", self.get_age_minutes(), age_threshold_minutes);
        self.get_volume_sol() >= volume_threshold
            && self.get_buyer_count() >= buyers_threshold
            && self.get_age_minutes() <= age_threshold_minutes
    }

    /// Cleans up old volume history entries
    pub fn cleanup_old_entries(&mut self, max_age_minutes: u64) {
        let cutoff_time = get_current_timestamp() - (max_age_minutes * 60 * 1000);
        self.volume_history.retain(|entry| entry.timestamp >= cutoff_time);
    }
}

/// Token metrics manager for tracking all tokens
#[derive(Debug)]
pub struct TokenMetricsManager {
    pub tokens: HashMap<String, TokenMetrics>,
}

impl TokenMetricsManager {
    /// Creates new token metrics manager
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    /// Updates or creates token metrics
    pub fn update_token(&mut self, token_mint: String, volume: u64, buyer: String, platform: String) {
        let metrics = self.tokens.entry(token_mint.clone()).or_insert_with(|| {
            TokenMetrics::new(token_mint, platform)
        });
        
        metrics.update(volume, buyer);
    }

    /// Gets token metrics by mint address
    pub fn get_token(&self, token_mint: &str) -> Option<&TokenMetrics> {
        self.tokens.get(token_mint)
    }

    /// Gets all tokens that meet spike criteria
    pub fn get_spike_tokens(&self, volume_threshold: f64, buyers_threshold: usize, age_threshold_minutes: u64) -> Vec<&TokenMetrics> {
        self.tokens
            .values()
            .filter(|token| token.is_spike(volume_threshold, buyers_threshold, age_threshold_minutes))
            .collect()
    }
    
    /// Gets all tokens that meet ratio-based spike criteria
    pub fn get_ratio_spike_tokens(&self, ratio_threshold: f64, time_window_minutes: u64) -> Vec<(&TokenMetrics, f64, u64)> {
        let mut result = Vec::new();
        
        for token in self.tokens.values() {
            if token.is_ratio_spike(ratio_threshold, time_window_minutes) {
                let (recent_sol, recent_tx_count) = token.get_recent_activity(time_window_minutes);
                result.push((token, recent_sol, recent_tx_count));
            }
        }
        
        result
    }

    /// Cleans up old tokens and their data
    pub fn cleanup_old_tokens(&mut self, max_age_minutes: u64) {
        let cutoff_time = get_current_timestamp() - (max_age_minutes * 60 * 1000);
        
        // Remove tokens that are too old
        self.tokens.retain(|_, token| token.first_seen >= cutoff_time);
        
        // Clean up old entries in remaining tokens
        for token in self.tokens.values_mut() {
            token.cleanup_old_entries(max_age_minutes);
        }
    }

    /// Gets total number of tracked tokens
    pub fn get_token_count(&self) -> usize {
        self.tokens.len()
    }
}

/// Gets current timestamp in milliseconds
pub fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Spike detection result
#[derive(Debug, Clone)]
pub struct SpikeDetectionResult {
    pub token_metrics: TokenMetrics,
    pub is_new_spike: bool,
    pub spike_score: f64,
    pub is_ratio_spike: bool,
    pub sol_per_buy_ratio: f64,
    pub recent_sol: f64,
    pub recent_buys: u64,
}

impl SpikeDetectionResult {
    /// Creates new spike detection result
    pub fn new(token_metrics: TokenMetrics, is_new_spike: bool) -> Self {
        let spike_score = calculate_spike_score(&token_metrics);
        let sol_per_buy_ratio = token_metrics.get_sol_per_buy_ratio();
        
        Self {
            token_metrics,
            is_new_spike,
            spike_score,
            is_ratio_spike: false, // Will be set later if applicable
            sol_per_buy_ratio,
            recent_sol: 0.0, // Will be set if this is a ratio spike
            recent_buys: 0, // Will be set if this is a ratio spike
        }
    }

    /// Marks this as a ratio-based spike with recent activity stats
    pub fn with_ratio_spike(mut self, recent_sol: f64, recent_buys: u64) -> Self {
        self.is_ratio_spike = true;
        self.recent_sol = recent_sol;
        self.recent_buys = recent_buys;
        self
    }
}

/// Calculates a spike score based on volume, buyers, and age
pub fn calculate_spike_score(token: &TokenMetrics) -> f64 {
    let volume_score = token.get_volume_sol();
    let buyer_score = token.get_buyer_count() as f64;
    let age_score = if token.get_age_minutes() > 0 {
        1.0 / token.get_age_minutes() as f64
    } else {
        1.0
    };
    
    // Weighted combination of factors
    (volume_score * 0.5) + (buyer_score * 0.3) + (age_score * 100.0 * 0.2)
}

/// Formats spike detection output
pub fn format_spike_output(spike: &SpikeDetectionResult) -> String {
    let token = &spike.token_metrics;
    
    let base_output = format!(
        "🔥 [HOT] {} | Volume: {} | Buyers: {} | Age: {} min | Platform: {} | Score: {:.2}",
        token.symbol,
        format_sol(token.total_volume),
        token.get_buyer_count(),
        token.get_age_minutes(),
        token.platform,
        spike.spike_score
    );
    
    // If it's a ratio spike, add the ratio information
    if spike.is_ratio_spike {
        format!(
            "{} | 🚨 RATIO SPIKE: {:.2} SOL/{} buys = {:.2} SOL/buy",
            base_output,
            spike.recent_sol,
            spike.recent_buys,
            spike.sol_per_buy_ratio
        )
    } else {
        base_output
    }
} 