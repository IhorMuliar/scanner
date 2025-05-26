# Database Setup Guide

This guide explains how to set up PostgreSQL for the Solana Token Scanner.

## Prerequisites

- PostgreSQL 12+ installed
- Database user with CREATE privileges

## Quick Setup

### 1. Install PostgreSQL

**macOS (using Homebrew):**

```bash
brew install postgresql
brew services start postgresql
```

**Ubuntu/Debian:**

```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

### 2. Create Database and User

```bash
# Connect to PostgreSQL as superuser
sudo -u postgres psql

# Create database
CREATE DATABASE solana_scanner;

# Create user with password
CREATE USER scanner_user WITH PASSWORD 'your_secure_password';

# Grant privileges
GRANT ALL PRIVILEGES ON DATABASE solana_scanner TO scanner_user;

# Exit psql
\q
```

### 3. Set Environment Variable

```bash
export DATABASE_URL="postgresql://scanner_user:your_secure_password@localhost:5432/solana_scanner"
```

Or create a `.env` file in the project root:
```
DATABASE_URL=postgresql://scanner_user:your_secure_password@localhost:5432/solana_scanner
```

### 4. Run the Scanner

The scanner will automatically run migrations on startup:

```bash
cargo run -- --database-url "postgresql://scanner_user:your_secure_password@localhost:5432/solana_scanner"
```

## Database Schema

The scanner creates the following tables:

### `tokens`
- Stores basic token information (mint, symbol, platform, first_seen)

### `token_metrics` 
- Stores aggregated metrics (volume, unique buyers, transaction count)

### `transactions`
- Stores individual transaction records with deduplication

### `hot_tokens`
- Stores detected hot tokens/spikes for quick querying

## Querying the Database

### Recent Hot Tokens
```sql
SELECT symbol, volume_sol, unique_buyers_count, platform, detected_at 
FROM hot_tokens 
WHERE detected_at >= NOW() - INTERVAL '1 hour'
ORDER BY volume_sol DESC;
```

### Top Tokens by Volume
```sql
SELECT t.symbol, tm.total_volume, tm.unique_buyers, tm.transaction_count
FROM tokens t
JOIN token_metrics tm ON t.mint = tm.token_mint
ORDER BY tm.total_volume DESC
LIMIT 10;
```

### Platform Activity
```sql
SELECT platform, COUNT(*) as token_count, SUM(total_volume) as total_volume
FROM tokens t
JOIN token_metrics tm ON t.mint = tm.token_mint
GROUP BY platform
ORDER BY total_volume DESC;
```

## Performance Considerations

- The database includes indexes on frequently queried fields
- Connection pooling is handled automatically by SQLx
- Consider setting up read replicas for high-traffic scenarios
- Monitor database size and implement archival strategies for old data

## Troubleshooting

### Connection Issues
- Verify PostgreSQL is running: `sudo systemctl status postgresql`
- Check connection string format
- Ensure user has proper permissions

### Migration Errors
- Check PostgreSQL logs: `sudo tail -f /var/log/postgresql/postgresql-*.log`
- Verify database exists and user has CREATE privileges
- Ensure no conflicting schema exists 