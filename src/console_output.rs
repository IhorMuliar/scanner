use crate::spike_detector::{HotToken, AlertLevel};
use chrono::{DateTime, Utc};
use std::time::SystemTime;

pub struct ConsoleOutput {
    // Could add formatting options, colors, etc.
}

impl ConsoleOutput {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn print_hot_token(&self, hot_token: &HotToken) {
        let emoji = match hot_token.alert_level {
            AlertLevel::Warm => "🌡️",
            AlertLevel::Hot => "🔥",
            AlertLevel::Blazing => "🚀",
        };
        
        let level_text = match hot_token.alert_level {
            AlertLevel::Warm => "WARM",
            AlertLevel::Hot => "HOT",
            AlertLevel::Blazing => "BLAZING",
        };
        
        let timestamp = self.format_timestamp();
        let mint_short = self.format_pubkey_short(&hot_token.mint);
        
        // Main alert line
        println!(
            "{} {} {} | {:.2} SOL | {} buyers | Age: {:.1} min | Score: {:.2} | Mint: {}",
            timestamp,
            emoji,
            level_text,
            hot_token.total_volume,
            hot_token.buyer_count,
            hot_token.age_minutes,
            hot_token.score,
            mint_short
        );
        
        // Additional details for hot/blazing tokens
        if matches!(hot_token.alert_level, AlertLevel::Hot | AlertLevel::Blazing) {
            println!(
                "    └─ Recent 5min: {:.2} SOL from {} buyers",
                hot_token.recent_volume_5min,
                hot_token.recent_buyers_5min
            );
        }
        
        // Add separator for blazing tokens
        if matches!(hot_token.alert_level, AlertLevel::Blazing) {
            println!("    ═══════════════════════════════════════════════════════════");
        }
        
        println!(); // Empty line for readability
    }
    
    pub fn print_startup_banner(&self) {
        println!("
╔═══════════════════════════════════════════════════════════════╗
║                    🔍 SOLANA DEGEN SCANNER 🔍                 ║
║                                                               ║
║  Monitoring: Pump.fun, Raydium, Meteora                      ║
║  Detection: Volume spikes, buyer surges, early signals       ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
        ");
        println!("Scanner started at {}", self.format_timestamp());
        println!("Watching for hot tokens...\n");
    }
    
    pub fn print_scan_summary(&self, tokens_tracked: usize, events_processed: usize, hot_tokens_found: usize) {
        println!(
            "📊 Scan Summary: {} tokens tracked | {} events processed | {} hot tokens detected",
            tokens_tracked, events_processed, hot_tokens_found
        );
    }
    
    pub fn print_error(&self, error: &str) {
        println!("❌ ERROR: {}", error);
    }
    
    pub fn print_info(&self, message: &str) {
        println!("ℹ️  {}", message);
    }
    
    fn format_timestamp(&self) -> String {
        let now = SystemTime::now();
        let datetime: DateTime<Utc> = now.into();
        datetime.format("%H:%M:%S").to_string()
    }
    
    fn format_pubkey_short(&self, pubkey: &solana_sdk::pubkey::Pubkey) -> String {
        let pubkey_str = pubkey.to_string();
        if pubkey_str.len() >= 8 {
            format!("{}...{}", &pubkey_str[..4], &pubkey_str[pubkey_str.len()-4..])
        } else {
            pubkey_str
        }
    }
}

// Utility functions for enhanced console output
pub fn print_table_header() {
    println!("┌─────────┬─────┬────────┬────────┬─────────┬─────────┬──────────────┐");
    println!("│  Time   │ Lvl │ Volume │ Buyers │ Age(min)│  Score  │    Mint      │");
    println!("├─────────┼─────┼────────┼────────┼─────────┼─────────┼──────────────┤");
}

pub fn print_table_footer() {
    println!("└─────────┴─────┴────────┴────────┴─────────┴─────────┴──────────────┘");
}

// Alternative compact format for high-frequency updates
impl ConsoleOutput {
    pub fn print_hot_token_compact(&self, hot_token: &HotToken) {
        let emoji = match hot_token.alert_level {
            AlertLevel::Warm => "🟡",
            AlertLevel::Hot => "🔥",
            AlertLevel::Blazing => "🚀",
        };
        
        println!(
            "{} {} {:>6.1}SOL {:>2}👥 {:>4.0}m {:>4.2}⭐ {}",
            self.format_timestamp(),
            emoji,
            hot_token.total_volume,
            hot_token.buyer_count,
            hot_token.age_minutes,
            hot_token.score,
            self.format_pubkey_short(&hot_token.mint)
        );
    }
    
    pub fn print_status_line(&self, slot: u64, tokens_tracked: usize) {
        print!("\r🔄 Slot: {} | 📈 Tracking: {} tokens", slot, tokens_tracked);
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
    }
} 