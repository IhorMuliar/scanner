use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Formats lamports to SOL with specified decimal places
pub fn format_sol(lamports: u64, decimals: usize) -> String {
    let sol = lamports as f64 / 1_000_000_000.0;
    format!("{:.decimals$} SOL", sol, decimals = decimals)
}

/// Validates if a string is a valid Solana public key
pub fn is_valid_public_key(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}

/// Safely parses a string to a Pubkey
pub fn parse_pubkey(address: &str) -> Result<Pubkey> {
    Pubkey::from_str(address).map_err(|e| anyhow::anyhow!("Invalid pubkey '{}': {}", address, e))
}

/// Sleep for the specified duration in milliseconds
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// Extracts the first few characters of a pubkey for display purposes
pub fn truncate_pubkey(pubkey: &Pubkey, length: usize) -> String {
    let pubkey_str = pubkey.to_string();
    if pubkey_str.len() <= length {
        pubkey_str
    } else {
        format!("{}...", &pubkey_str[0..length])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sol() {
        assert_eq!(format_sol(1_000_000_000, 2), "1.00 SOL");
        assert_eq!(format_sol(500_000_000, 2), "0.50 SOL");
        assert_eq!(format_sol(1_500_000_000, 2), "1.50 SOL");
    }

    #[test]
    fn test_is_valid_public_key() {
        assert!(is_valid_public_key("11111111111111111111111111111112"));
        assert!(!is_valid_public_key("invalid_key"));
        assert!(!is_valid_public_key(""));
    }

    #[test]
    fn test_truncate_pubkey() {
        let pubkey = Pubkey::from_str("11111111111111111111111111111112").unwrap();
        assert_eq!(truncate_pubkey(&pubkey, 4), "1111...");
        assert_eq!(truncate_pubkey(&pubkey, 50), pubkey.to_string());
    }
} 