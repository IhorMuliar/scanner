use crate::token_tracker::{TokenTracker, TokenStats};
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};
use log::debug;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotToken {
    pub mint: Pubkey,
    pub total_volume: f64,
    pub buyer_count: usize,
    pub age_minutes: f64,
    pub recent_volume_5min: f64,
    pub recent_buyers_5min: usize,
    pub score: f64,
    pub alert_level: AlertLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    Warm,      // Early activity detected
    Hot,       // Strong activity
    Blazing,   // Explosive activity
}

#[derive(Debug, Clone)]
pub struct SpikeThresholds {
    pub min_volume: f64,           // Minimum total volume to consider
    pub min_buyers: usize,         // Minimum unique buyers
    pub max_age_minutes: f64,      // Maximum age to consider "new"
    pub warm_volume: f64,          // Volume threshold for "warm" 
    pub hot_volume: f64,           // Volume threshold for "hot"
    pub blazing_volume: f64,       // Volume threshold for "blazing"
    pub warm_buyers: usize,        // Buyer threshold for "warm"
    pub hot_buyers: usize,         // Buyer threshold for "hot"
    pub blazing_buyers: usize,     // Buyer threshold for "blazing"
    pub recent_window_seconds: u64, // Time window for recent activity analysis
}

impl Default for SpikeThresholds {
    fn default() -> Self {
        Self {
            min_volume: 2.0,           // At least 2 SOL total
            min_buyers: 3,             // At least 3 unique buyers
            max_age_minutes: 60.0,     // Only consider tokens < 1 hour old
            warm_volume: 5.0,          // 5+ SOL for warm
            hot_volume: 15.0,          // 15+ SOL for hot
            blazing_volume: 50.0,      // 50+ SOL for blazing
            warm_buyers: 5,            // 5+ buyers for warm
            hot_buyers: 10,            // 10+ buyers for hot
            blazing_buyers: 20,        // 20+ buyers for blazing
            recent_window_seconds: 300, // Look at last 5 minutes
        }
    }
}

pub struct SpikeDetector {
    thresholds: SpikeThresholds,
    last_alert_times: std::collections::HashMap<Pubkey, u64>, // Prevent spam
}

impl SpikeDetector {
    pub fn new() -> Self {
        Self {
            thresholds: SpikeThresholds::default(),
            last_alert_times: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_thresholds(thresholds: SpikeThresholds) -> Self {
        Self {
            thresholds,
            last_alert_times: std::collections::HashMap::new(),
        }
    }
    
    pub fn detect_hot_tokens(&mut self, token_tracker: &TokenTracker) -> Vec<HotToken> {
        let current_timestamp = self.get_current_timestamp();
        let mut hot_tokens = Vec::new();
        
        // Get active tokens within age limit
        let active_tokens = token_tracker.get_active_tokens(
            current_timestamp, 
            self.thresholds.max_age_minutes
        );
        
        for token_stats in active_tokens {
            if let Some(hot_token) = self.evaluate_token(token_stats, current_timestamp) {
                // Check if we've alerted for this token recently (prevent spam)
                if self.should_alert(&hot_token.mint, current_timestamp) {
                    let mint = hot_token.mint; // Extract mint before moving hot_token
                    hot_tokens.push(hot_token);
                    self.last_alert_times.insert(mint, current_timestamp);
                }
            }
        }
        
        // Sort by score (highest first)
        hot_tokens.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        debug!("Detected {} hot tokens", hot_tokens.len());
        hot_tokens
    }
    
    fn evaluate_token(&self, token_stats: &TokenStats, current_timestamp: u64) -> Option<HotToken> {
        let buyer_count = token_stats.buyer_count();
        let age_minutes = token_stats.age_minutes(current_timestamp);
        
        // Apply minimum thresholds
        if token_stats.total_volume < self.thresholds.min_volume 
            || buyer_count < self.thresholds.min_buyers {
            return None;
        }
        
        // Calculate recent activity (last 5 minutes)
        let recent_volume = token_stats.recent_volume_spike(
            self.thresholds.recent_window_seconds, 
            current_timestamp
        );
        let recent_buyers = token_stats.recent_buyer_surge(
            self.thresholds.recent_window_seconds,
            current_timestamp
        );
        
        // Calculate composite score
        let score = self.calculate_score(
            token_stats.total_volume,
            buyer_count,
            age_minutes,
            recent_volume,
            recent_buyers
        );
        
        // Determine alert level
        let alert_level = self.determine_alert_level(
            token_stats.total_volume,
            buyer_count,
            recent_volume,
            recent_buyers
        );
        
        Some(HotToken {
            mint: token_stats.mint,
            total_volume: token_stats.total_volume,
            buyer_count,
            age_minutes,
            recent_volume_5min: recent_volume,
            recent_buyers_5min: recent_buyers,
            score,
            alert_level,
        })
    }
    
    fn calculate_score(
        &self,
        total_volume: f64,
        buyer_count: usize,
        age_minutes: f64,
        recent_volume: f64,
        recent_buyers: usize
    ) -> f64 {
        // Weighted scoring system
        let volume_score = (total_volume / self.thresholds.hot_volume).min(5.0); // Cap at 5x
        let buyer_score = (buyer_count as f64 / self.thresholds.hot_buyers as f64).min(3.0); // Cap at 3x
        let recency_score = if age_minutes > 0.0 { 
            (60.0 / age_minutes).min(10.0) // More recent = higher score, cap at 10x
        } else { 
            10.0 
        };
        let recent_activity_score = (recent_volume / 5.0) + (recent_buyers as f64 / 5.0);
        
        // Weighted combination
        (volume_score * 0.3) + 
        (buyer_score * 0.3) + 
        (recency_score * 0.2) + 
        (recent_activity_score * 0.2)
    }
    
    fn determine_alert_level(
        &self,
        total_volume: f64,
        buyer_count: usize,
        recent_volume: f64,
        recent_buyers: usize
    ) -> AlertLevel {
        let is_blazing = total_volume >= self.thresholds.blazing_volume 
            || buyer_count >= self.thresholds.blazing_buyers
            || recent_volume >= self.thresholds.blazing_volume * 0.5
            || recent_buyers >= self.thresholds.blazing_buyers / 2;
            
        let is_hot = total_volume >= self.thresholds.hot_volume 
            || buyer_count >= self.thresholds.hot_buyers
            || recent_volume >= self.thresholds.hot_volume * 0.5
            || recent_buyers >= self.thresholds.hot_buyers / 2;
            
        if is_blazing {
            AlertLevel::Blazing
        } else if is_hot {
            AlertLevel::Hot
        } else {
            AlertLevel::Warm
        }
    }
    
    fn should_alert(&self, mint: &Pubkey, current_timestamp: u64) -> bool {
        // Don't spam alerts - minimum 2 minutes between alerts for same token
        let min_interval = 120; // 2 minutes
        
        match self.last_alert_times.get(mint) {
            Some(last_time) => current_timestamp - last_time >= min_interval,
            None => true,
        }
    }
    
    fn get_current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
} 