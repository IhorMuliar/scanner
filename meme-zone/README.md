# Pump.fun Token Lifecycle Monitor

A Rust-based real-time monitoring system for Pump.fun tokens that tracks their lifecycle from creation to graduation (migration to Raydium).

## Features

### ✅ Implemented

1. **New Token Creation Monitoring**
   - Detects `CREATE` instructions from Pump.fun program
   - Logs token mint addresses and creation events
   - Tracks creator information

2. **Token Buy Activity Tracking**
   - Monitors `BUY` instructions to track trading activity
   - Calculates SOL amounts spent on tokens
   - Identifies "hot" tokens based on configurable thresholds

3. **Token Migration Detection**
   - Detects `MIGRATE` instructions when tokens graduate to Raydium
   - Tracks which tokens have successfully migrated
   - Records migration timestamps

4. **Hot Token Detection**
   - Configurable SOL spending threshold (default: 5 SOL)
   - Configurable buy count threshold (default: 5 buys)
   - Time-window based tracking (default: 1 hour)
   - Real-time alerts when tokens become "hot"

5. **About-to-Graduate Token Monitoring** 🎯
   - **Bonding Curve Progress Calculation**: Uses the formula `((1_073_000_000 * 10^6) - virtualTokenReserves) * 100 / (793_100_000 * 10^6)` to calculate graduation progress
   - **Graduation Thresholds**: Alerts at 50%, 70%, 80%, 90%, 95%, and 99% completion
   - **Market Cap Calculation**: Real-time market cap calculation using bonding curve pricing
   - **SOL Requirements**: Tracks how much SOL is needed to reach graduation (85 SOL threshold)
   - **Multi-level Alerts**: Progressive warning system as tokens approach graduation

### 🔧 Bonding Curve Monitoring

The system implements comprehensive bonding curve state tracking:

```rust
/// Bonding curve constants for graduation calculations
const INITIAL_VIRTUAL_TOKEN_RESERVES: u64 = 1_073_000_000_000_000; // 1.073T tokens
const TOTAL_SELLABLE_TOKENS: u64 = 793_100_000_000_000; // 793.1B sellable tokens  
const GRADUATION_SOL_THRESHOLD: f64 = 85.0; // 85 SOL for graduation
```

**Progress Calculation Formula:**
```
graduation_progress = ((INITIAL_VIRTUAL_TOKEN_RESERVES - virtual_token_reserves) * 100) / TOTAL_SELLABLE_TOKENS
```

**Graduation Alert Thresholds:**
- 🟡 50% - Initial graduation tracking
- 🟠 70% - Moderate progress alert
- 🔶 80% - High progress alert  
- 🔴 90% - Critical progress alert
- 🚨 95% - Very close to graduation
- ⚠️ 99% - Imminent graduation

### 📊 Statistics & Reporting

The monitor provides comprehensive real-time statistics:

- **Scanner Performance**: Blocks processed, transactions found
- **Token Activity**: Total tokens tracked, hot tokens detected
- **Migration Status**: Number of migrated tokens
- **Graduation Progress**: Tokens close to graduation with progress percentages
- **Market Data**: Real-time market cap calculations for tracked tokens

## Usage

### Basic Usage

```bash
cargo run
```

### Advanced Configuration

```bash
cargo run -- \
  --rpc-url "https://api.mainnet-beta.solana.com" \
  --interval-ms 500 \
  --hot-sol-threshold 10.0 \
  --hot-buys-threshold 10 \
  --hot-time-window 7200
```

### Command Line Options

- `--rpc-url`: Solana RPC endpoint (default: mainnet-beta)
- `--interval-ms`: Polling interval in milliseconds (default: 1000ms)
- `--start-slot`: Starting slot number (0 for latest)
- `--max-blocks`: Maximum blocks to process (0 for unlimited)
- `--hot-sol-threshold`: Minimum SOL for hot token detection (default: 5.0)
- `--hot-buys-threshold`: Minimum buys for hot token detection (default: 5)
- `--hot-time-window`: Time window for tracking in seconds (default: 3600)

## Output Examples

### Hot Token Detection
```
🔥 HOT TOKEN DETECTED: 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
  Total SOL spent: 12.45 SOL
  Buy count: 8
  First seen: 2024-01-15T10:30:00Z
  Age: 1250s
```

### Graduation Progress Alerts
```
🎯 GRADUATION ALERT (90%): 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
  Graduation progress: 92.34%
  Current market cap: 78.456 SOL
  SOL collected: 78.45 / 85.0
  SOL needed: 6.55
  Virtual token reserves: 60871172877861
  Real token reserves: 40271172877861
  🚨 CRITICAL: Token is very close to graduation!
```

### Periodic Statistics
```
📈 SCANNER STATISTICS:
  Blocks processed: 1234
  Transactions processed: 567
  Hot tokens detected: 89
  Migrated tokens: 12

🎯 GRADUATION TRACKING:
  Total tokens tracked: 45
  Close to graduation (>50%): 8  
  Completed tokens: 3

🔥 Active hot tokens: 5
  1. 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU
  2. 8yLYth3FD98k6f8743HDjhDd7jFjk8hGjDk9LpMnBcVx
  ...

🎯 Tokens close to graduation:
  1. 9zMWts4GE12k7f8743HDjhDd7jFjk8hGjDk9LpMnBcVx - 87.2% progress, 74.234 SOL market cap
  2. AaMWts4GE12k7f8743HDjhDd7jFjk8hGjDk9LpMnBcVx - 65.8% progress, 45.123 SOL market cap
  ...
```

## Architecture

### Core Components

1. **SolanaBlockScanner**: Main scanning engine
2. **TokenTracker**: Hot token detection and tracking
3. **GraduationTracker**: Bonding curve progress monitoring
4. **BondingCurveState**: State management for graduation calculations

### Data Structures

- **TokenData**: Comprehensive token activity tracking
- **BondingCurveState**: Real-time bonding curve state
- **ProcessedTransaction**: Transaction metadata and analysis
- **TokenTransaction**: Individual transaction records

## Technical Implementation

### Instruction Monitoring

The system monitors specific Pump.fun instruction discriminators:
- `CREATE`: `[24, 30, 200, 40, 5, 28, 7, 119]`
- `BUY`: `[102, 6, 61, 18, 1, 218, 235, 234]`
- `MIGRATE`: `[155, 234, 231, 146, 236, 158, 162, 30]`

### Bonding Curve Analysis

Currently implemented using balance changes and transaction analysis. Future enhancements will include:
- Direct TradeEvent log parsing
- Real-time bonding curve account monitoring
- Advanced graduation prediction algorithms

### Performance Features

- **Concurrent Processing**: Async/await based architecture
- **Memory Management**: Automatic cleanup of old token data
- **Rate Limiting**: Configurable polling intervals
- **Error Recovery**: Robust error handling and retry logic

## Dependencies

- `solana-client`: RPC connections and blockchain interaction
- `tokio`: Async runtime for concurrent processing
- `dashmap`: Thread-safe concurrent hash maps
- `chrono`: Timestamp handling and date calculations
- `clap`: Command-line argument parsing
- `log/env_logger`: Structured logging framework

## Future Enhancements

### Planned Features

1. **Advanced Log Parsing**: Direct TradeEvent extraction from transaction logs
2. **Account Subscription**: Real-time bonding curve account monitoring  
3. **Graduation Prediction**: ML-based graduation timing prediction
4. **Performance Metrics**: Buy velocity and momentum analysis
5. **Database Integration**: Persistent storage for historical analysis
6. **API Interface**: REST API for external integrations
7. **WebSocket Streaming**: Real-time data streaming capabilities

### Integration Opportunities

- Discord/Telegram notification bots
- Trading strategy automation
- Portfolio tracking applications
- Market analysis dashboards

## Configuration

The system uses environment variables and command-line arguments for configuration. Create a `.env` file for persistent settings:

```env
RUST_LOG=info
RPC_URL=https://api.mainnet-beta.solana.com
HOT_SOL_THRESHOLD=5.0
HOT_BUYS_THRESHOLD=5
HOT_TIME_WINDOW=3600
```

## Monitoring Best Practices

1. **Start with Default Settings**: Use default thresholds initially
2. **Monitor Resource Usage**: Watch memory and CPU consumption  
3. **Adjust Thresholds**: Fine-tune based on market conditions
4. **Log Analysis**: Review periodic statistics for insights
5. **Error Monitoring**: Monitor logs for RPC connection issues

## Contributing

This project implements the core about-to-graduate token monitoring functionality as specified. The bonding curve progress calculation uses the exact formula provided and tracks graduation thresholds accurately.

For advanced features like real-time log parsing, additional Solana SDK expertise may be required to handle the `OptionSerializer` complexity in transaction logs. 