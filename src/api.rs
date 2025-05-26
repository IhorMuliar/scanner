use crate::database::DatabaseService;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API response for hot tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct HotTokenResponse {
    pub symbol: String,
    pub volume_sol: f64,
    pub unique_buyers_count: i32,
    pub age_minutes: i32,
    pub platform: String,
    pub detected_at: String,
}

/// API response for token metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenMetricsResponse {
    pub symbol: String,
    pub total_volume: i64,
    pub unique_buyers: i32,
    pub transaction_count: i32,
    pub platform: String,
}

/// API service for querying scanner data
pub struct ApiService {
    database: Arc<DatabaseService>,
}

impl ApiService {
    pub fn new(database: Arc<DatabaseService>) -> Self {
        Self { database }
    }

    /// Get recent hot tokens
    pub async fn get_recent_hot_tokens(&self, hours: i32, limit: i32) -> Result<Vec<HotTokenResponse>> {
        let hot_tokens = self.database.get_recent_hot_tokens(limit, hours).await?;
        
        Ok(hot_tokens
            .into_iter()
            .map(|token| HotTokenResponse {
                symbol: token.symbol,
                volume_sol: token.volume_sol,
                unique_buyers_count: token.unique_buyers_count,
                age_minutes: token.age_minutes,
                platform: platform_to_string(&token.platform),
                detected_at: token.detected_at.to_rfc3339(),
            })
            .collect())
    }

    /// Health check endpoint
    pub async fn health_check(&self) -> Result<()> {
        self.database.health_check().await
    }
}

/// Convert Platform enum to string for API responses
fn platform_to_string(platform: &crate::utils::Platform) -> String {
    platform.as_str().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_conversion() {
        use crate::utils::Platform;
        
        assert_eq!(platform_to_string(&Platform::PumpFun), "Pump.fun");
        assert_eq!(platform_to_string(&Platform::Raydium), "Raydium");
        assert_eq!(platform_to_string(&Platform::Meteora), "Meteora");
        assert_eq!(platform_to_string(&Platform::Unknown), "Unknown");
    }
} 