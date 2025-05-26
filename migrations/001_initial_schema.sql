-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create platform enum
CREATE TYPE platform AS ENUM ('PUMP_FUN', 'RAYDIUM', 'METEORA', 'UNKNOWN');

-- Create tokens table
CREATE TABLE tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mint VARCHAR(44) UNIQUE NOT NULL,
    symbol VARCHAR(50),
    platform platform NOT NULL,
    first_seen TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create token_metrics table
CREATE TABLE token_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_mint VARCHAR(44) NOT NULL REFERENCES tokens(mint) ON DELETE CASCADE,
    total_volume BIGINT NOT NULL DEFAULT 0,
    unique_buyers INTEGER NOT NULL DEFAULT 0,
    transaction_count INTEGER NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ DEFAULT NOW()
);

-- Create transactions table
CREATE TABLE transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    signature VARCHAR(88) UNIQUE NOT NULL,
    token_mint VARCHAR(44) NOT NULL REFERENCES tokens(mint) ON DELETE CASCADE,
    buyer VARCHAR(44) NOT NULL,
    volume BIGINT NOT NULL,
    platform platform NOT NULL,
    block_time TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create hot_tokens table  
CREATE TABLE hot_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_mint VARCHAR(44) NOT NULL REFERENCES tokens(mint) ON DELETE CASCADE,
    symbol VARCHAR(50) NOT NULL,
    volume_sol DECIMAL(20, 9) NOT NULL,
    unique_buyers_count INTEGER NOT NULL,
    age_minutes INTEGER NOT NULL,
    platform platform NOT NULL,
    detected_at TIMESTAMPTZ DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX idx_tokens_mint ON tokens(mint);
CREATE INDEX idx_tokens_platform ON tokens(platform);
CREATE INDEX idx_tokens_first_seen ON tokens(first_seen);

CREATE INDEX idx_token_metrics_token_mint ON token_metrics(token_mint);
CREATE INDEX idx_token_metrics_last_updated ON token_metrics(last_updated);

CREATE INDEX idx_transactions_token_mint ON transactions(token_mint);
CREATE INDEX idx_transactions_platform ON transactions(platform);
CREATE INDEX idx_transactions_block_time ON transactions(block_time);
CREATE INDEX idx_transactions_buyer ON transactions(buyer);

CREATE INDEX idx_hot_tokens_detected_at ON hot_tokens(detected_at);
CREATE INDEX idx_hot_tokens_platform ON hot_tokens(platform);
CREATE INDEX idx_hot_tokens_volume_sol ON hot_tokens(volume_sol);
CREATE INDEX idx_hot_tokens_token_mint ON hot_tokens(token_mint);

-- Function to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Trigger for tokens table
CREATE TRIGGER update_tokens_updated_at BEFORE UPDATE ON tokens
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column(); 