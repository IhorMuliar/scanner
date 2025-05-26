use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log::info;
use solana_sdk::pubkey::Pubkey;
use sqlx::{PgPool, Row};

use uuid::Uuid;

use crate::utils::{HotToken, Platform, TokenInfo, TokenMetrics};

/// Database service for token scanner
#[derive(Clone)]
pub struct DatabaseService {
    pool: PgPool,
}

/// Database model structs
#[derive(Debug, Clone)]
pub struct DbToken {
    pub id: Uuid,
    pub mint: String,
    pub symbol: Option<String>,
    pub platform: Platform,
    pub first_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DbTokenMetric {
    pub id: Uuid,
    pub token_mint: String,
    pub total_volume: i64,
    pub unique_buyers: i32,
    pub transaction_count: i32,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DbTransaction {
    pub id: Uuid,
    pub signature: String,
    pub token_mint: String,
    pub buyer: String,
    pub volume: i64,
    pub platform: Platform,
    pub block_time: DateTime<Utc>,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DbHotToken {
    pub id: Uuid,
    pub token_mint: String,
    pub symbol: String,
    pub volume_sol: f64,
    pub unique_buyers_count: i32,
    pub age_minutes: i32,
    pub platform: Platform,
    pub detected_at: DateTime<Utc>,
}

impl DatabaseService {
    /// Creates a new database service instance
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("🔌 Connecting to database...");
        
        let pool = PgPool::connect(database_url)
            .await
            .context("Failed to connect to database")?;

        info!("✅ Database connected successfully");
        
        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("🔄 Running database migrations...");
        
        // Read and execute migration file
        let migration_sql = include_str!("../migrations/001_initial_schema.sql");
        
        sqlx::query(migration_sql)
            .execute(&self.pool)
            .await
            .context("Failed to run migrations")?;

        info!("✅ Database migrations completed");
        Ok(())
    }

    /// Insert or update a token
    pub async fn upsert_token(&self, token_info: &TokenInfo, first_seen: DateTime<Utc>, symbol: Option<String>) -> Result<DbToken> {
        let platform_str = platform_to_db_string(&token_info.platform);
        let mint_str = token_info.token_mint.to_string();

        let row = sqlx::query(
            r#"
            INSERT INTO tokens (mint, symbol, platform, first_seen)
            VALUES ($1, $2, $3::platform, $4)
            ON CONFLICT (mint) 
            DO UPDATE SET 
                symbol = COALESCE(EXCLUDED.symbol, tokens.symbol),
                updated_at = NOW()
            RETURNING id, mint, symbol, platform, first_seen, created_at, updated_at
            "#
        )
        .bind(&mint_str)
        .bind(&symbol)
        .bind(platform_str)
        .bind(first_seen)
        .fetch_one(&self.pool)
        .await
        .context("Failed to upsert token")?;

        Ok(DbToken {
            id: row.get("id"),
            mint: row.get("mint"),
            symbol: row.get("symbol"),
            platform: db_string_to_platform(row.get("platform")),
            first_seen: row.get("first_seen"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Insert a transaction (with deduplication)
    pub async fn insert_transaction(&self, token_info: &TokenInfo, signature: &str, block_time: DateTime<Utc>) -> Result<Option<DbTransaction>> {
        let platform_str = platform_to_db_string(&token_info.platform);
        let mint_str = token_info.token_mint.to_string();
        let buyer_str = token_info.buyer.to_string();

        let result = sqlx::query(
            r#"
            INSERT INTO transactions (signature, token_mint, buyer, volume, platform, block_time)
            VALUES ($1, $2, $3, $4, $5::platform, $6)
            ON CONFLICT (signature) DO NOTHING
            RETURNING id, signature, token_mint, buyer, volume, platform, block_time, processed_at
            "#
        )
        .bind(signature)
        .bind(&mint_str)
        .bind(&buyer_str)
        .bind(token_info.volume as i64)
        .bind(platform_str)
        .bind(block_time)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to insert transaction")?;

        if let Some(row) = result {
            Ok(Some(DbTransaction {
                id: row.get("id"),
                signature: row.get("signature"),
                token_mint: row.get("token_mint"),
                buyer: row.get("buyer"),
                volume: row.get("volume"),
                platform: db_string_to_platform(row.get("platform")),
                block_time: row.get("block_time"),
                processed_at: row.get("processed_at"),
            }))
        } else {
            // Transaction already exists (duplicate)
            Ok(None)
        }
    }

    /// Update token metrics
    pub async fn upsert_token_metrics(&self, metrics: &TokenMetrics) -> Result<DbTokenMetric> {
        let mint_str = metrics.mint.to_string();
        let unique_buyers_count = metrics.unique_buyers_count() as i32;

        let row = sqlx::query(
            r#"
            INSERT INTO token_metrics (token_mint, total_volume, unique_buyers, transaction_count)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (token_mint)
            DO UPDATE SET
                total_volume = EXCLUDED.total_volume,
                unique_buyers = EXCLUDED.unique_buyers,
                transaction_count = EXCLUDED.transaction_count,
                last_updated = NOW()
            RETURNING id, token_mint, total_volume, unique_buyers, transaction_count, last_updated
            "#
        )
        .bind(&mint_str)
        .bind(metrics.total_volume as i64)
        .bind(unique_buyers_count)
        .bind(metrics.transactions as i32)
        .fetch_one(&self.pool)
        .await
        .context("Failed to upsert token metrics")?;

        Ok(DbTokenMetric {
            id: row.get("id"),
            token_mint: row.get("token_mint"),
            total_volume: row.get("total_volume"),
            unique_buyers: row.get("unique_buyers"),
            transaction_count: row.get("transaction_count"),
            last_updated: row.get("last_updated"),
        })
    }

    /// Insert a detected hot token
    pub async fn insert_hot_token(&self, hot_token: &HotToken) -> Result<DbHotToken> {
        let platform_str = platform_to_db_string(&hot_token.platform);
        let mint_str = hot_token.mint.to_string();

        let row = sqlx::query(
            r#"
            INSERT INTO hot_tokens (token_mint, symbol, volume_sol, unique_buyers_count, age_minutes, platform)
            VALUES ($1, $2, $3, $4, $5, $6::platform)
            RETURNING id, token_mint, symbol, volume_sol, unique_buyers_count, age_minutes, platform, detected_at
            "#
        )
        .bind(&mint_str)
        .bind(&hot_token.symbol)
        .bind(hot_token.volume_in_sol)
        .bind(hot_token.unique_buyers_count as i32)
        .bind(hot_token.age_minutes as i32)
        .bind(platform_str)
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert hot token")?;

        Ok(DbHotToken {
            id: row.get("id"),
            token_mint: row.get("token_mint"),
            symbol: row.get("symbol"),
            volume_sol: row.get("volume_sol"),
            unique_buyers_count: row.get("unique_buyers_count"),
            age_minutes: row.get("age_minutes"),
            platform: db_string_to_platform(row.get("platform")),
            detected_at: row.get("detected_at"),
        })
    }

    /// Get recent hot tokens for API/frontend
    pub async fn get_recent_hot_tokens(&self, limit: i32, hours: i32) -> Result<Vec<DbHotToken>> {
        let rows = sqlx::query(
            r#"
            SELECT id, token_mint, symbol, volume_sol, unique_buyers_count, age_minutes, platform, detected_at
            FROM hot_tokens
            WHERE detected_at >= NOW() - INTERVAL '%1 hours'
            ORDER BY detected_at DESC, volume_sol DESC
            LIMIT $2
            "#
        )
        .bind(hours)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch recent hot tokens")?;

        Ok(rows.into_iter().map(|row| DbHotToken {
            id: row.get("id"),
            token_mint: row.get("token_mint"),
            symbol: row.get("symbol"),
            volume_sol: row.get("volume_sol"),
            unique_buyers_count: row.get("unique_buyers_count"),
            age_minutes: row.get("age_minutes"),
            platform: db_string_to_platform(row.get("platform")),
            detected_at: row.get("detected_at"),
        }).collect())
    }

    /// Get token metrics by mint
    pub async fn get_token_metrics(&self, mint: &Pubkey) -> Result<Option<DbTokenMetric>> {
        let mint_str = mint.to_string();

        let row = sqlx::query(
            r#"
            SELECT id, token_mint, total_volume, unique_buyers, transaction_count, last_updated
            FROM token_metrics
            WHERE token_mint = $1
            "#
        )
        .bind(&mint_str)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch token metrics")?;

        if let Some(row) = row {
            Ok(Some(DbTokenMetric {
                id: row.get("id"),
                token_mint: row.get("token_mint"),
                total_volume: row.get("total_volume"),
                unique_buyers: row.get("unique_buyers"),
                transaction_count: row.get("transaction_count"),
                last_updated: row.get("last_updated"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if transaction already exists (for deduplication)
    pub async fn transaction_exists(&self, signature: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM transactions WHERE signature = $1"
        )
        .bind(signature)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check transaction existence")?;

        Ok(count > 0)
    }

    /// Get database connection health
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .context("Database health check failed")?;
        
        Ok(())
    }

    /// Close database connection
    pub async fn close(&self) {
        self.pool.close().await;
        info!("📴 Database connection closed");
    }
}

/// Convert Platform enum to database string
fn platform_to_db_string(platform: &Platform) -> &'static str {
    match platform {
        Platform::PumpFun => "PUMP_FUN",
        Platform::Raydium => "RAYDIUM", 
        Platform::Meteora => "METEORA",
        Platform::Unknown => "UNKNOWN",
    }
}

/// Convert database string to Platform enum
fn db_string_to_platform(platform_str: &str) -> Platform {
    match platform_str {
        "PUMP_FUN" => Platform::PumpFun,
        "RAYDIUM" => Platform::Raydium,
        "METEORA" => Platform::Meteora,
        _ => Platform::Unknown,
    }
} 