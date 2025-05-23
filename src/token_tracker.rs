use std::collections::{HashMap, HashSet};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use crate::transaction_parser::TokenEvent;
use log::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStats {
    pub mint: Pubkey,
    pub total_volume: f64,
    pub unique_buyers: HashSet<Pubkey>,
    pub first_seen: u64,
    pub last_seen: u64,
    pub event_count: u32,
    pub recent_events: Vec<TokenEvent>, // Keep last 10 events for analysis
}

impl TokenStats {
    pub fn new(mint: Pubkey, timestamp: u64) -> Self {
        Self {
            mint,
            total_volume: 0.0,
            unique_buyers: HashSet::new(),
            first_seen: timestamp,
            last_seen: timestamp,
            event_count: 0,
            recent_events: Vec::new(),
        }
    }
    
    pub fn update(&mut self, event: &TokenEvent) {
        self.total_volume += event.sol_amount;
        self.unique_buyers.insert(event.buyer);
        self.last_seen = event.timestamp.max(self.last_seen);
        self.event_count += 1;
        
        // Keep only last 10 events
        self.recent_events.push(event.clone());
        if self.recent_events.len() > 10 {
            self.recent_events.remove(0);
        }
        
        debug!(
            "Updated token {} - Volume: {:.2} SOL, Buyers: {}, Events: {}",
            self.mint, self.total_volume, self.unique_buyers.len(), self.event_count
        );
    }
    
    pub fn buyer_count(&self) -> usize {
        self.unique_buyers.len()
    }
    
    pub fn age_seconds(&self, current_timestamp: u64) -> u64 {
        current_timestamp.saturating_sub(self.first_seen)
    }
    
    pub fn age_minutes(&self, current_timestamp: u64) -> f64 {
        self.age_seconds(current_timestamp) as f64 / 60.0
    }
    
    pub fn recent_volume_spike(&self, time_window_seconds: u64, current_timestamp: u64) -> f64 {
        let cutoff_time = current_timestamp.saturating_sub(time_window_seconds);
        
        self.recent_events
            .iter()
            .filter(|event| event.timestamp >= cutoff_time)
            .map(|event| event.sol_amount)
            .sum()
    }
    
    pub fn recent_buyer_surge(&self, time_window_seconds: u64, current_timestamp: u64) -> usize {
        let cutoff_time = current_timestamp.saturating_sub(time_window_seconds);
        
        let recent_buyers: HashSet<Pubkey> = self.recent_events
            .iter()
            .filter(|event| event.timestamp >= cutoff_time)
            .map(|event| event.buyer)
            .collect();
            
        recent_buyers.len()
    }
}

pub struct TokenTracker {
    tokens: HashMap<Pubkey, TokenStats>,
    max_tokens: usize,
}

impl TokenTracker {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            max_tokens: 10000, // Limit memory usage
        }
    }
    
    pub fn update(&mut self, event: &TokenEvent) {
        // Clean up old tokens if we hit the limit
        if self.tokens.len() >= self.max_tokens {
            self.cleanup_old_tokens(event.timestamp);
        }
        
        let token_stats = self.tokens
            .entry(event.mint)
            .or_insert_with(|| TokenStats::new(event.mint, event.timestamp));
            
        token_stats.update(event);
    }
    
    pub fn get_token_stats(&self, mint: &Pubkey) -> Option<&TokenStats> {
        self.tokens.get(mint)
    }
    
    pub fn get_all_tokens(&self) -> &HashMap<Pubkey, TokenStats> {
        &self.tokens
    }
    
    pub fn get_active_tokens(&self, current_timestamp: u64, max_age_minutes: f64) -> Vec<&TokenStats> {
        self.tokens
            .values()
            .filter(|stats| stats.age_minutes(current_timestamp) <= max_age_minutes)
            .collect()
    }
    
    fn cleanup_old_tokens(&mut self, current_timestamp: u64) {
        // Remove tokens older than 1 hour with low activity
        let cutoff_age = 3600; // 1 hour in seconds
        let min_volume = 1.0; // Minimum 1 SOL volume to keep
        
        self.tokens.retain(|_mint, stats| {
            let age = stats.age_seconds(current_timestamp);
            age < cutoff_age || stats.total_volume >= min_volume
        });
        
        debug!("Cleaned up old tokens, now tracking {} tokens", self.tokens.len());
    }
    
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
} 