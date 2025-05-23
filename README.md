# Solana Degen Scanner 🔍

A real-time Solana blockchain scanner that monitors token activity on Pump.fun, Raydium, and Meteora to detect early signals of "degen" behavior like rapid buys, high SOL inflows, and sudden volume spikes.

## 🚀 Features

- **Real-time Block Scanning**: Monitors Solana blockchain via RPC for new blocks and transactions
- **Multi-DEX Support**: Tracks activity on Pump.fun, Raydium, and Meteora
- **Smart Detection**: Identifies volume spikes, buyer surges, and early token activity
- **Alert Levels**: Categorizes tokens as Warm 🌡️, Hot 🔥, or Blazing 🚀
- **Beautiful Console Output**: Rich formatting with emojis and real-time updates
- **Memory Efficient**: In-memory tracking with automatic cleanup of old tokens

## 📋 Prerequisites

- Rust 1.70+ 
- Access to Solana RPC endpoint (mainnet-beta)

## 🛠️ Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd solana-scanner
```

2. Install dependencies:
```bash
cargo build --release
```

3. Set environment variables (optional):
```bash
export RUST_LOG=info  # Set log level (debug, info, warn, error)
```

## 🎯 Usage

### Basic Usage

Run the scanner with default settings:
```bash
cargo run --release
```

### With Debug Logging

For more verbose output during development:
```bash
RUST_LOG=debug cargo run --release
```

### Example Output

```
╔═══════════════════════════════════════════════════════════════╗
║                    🔍 SOLANA DEGEN SCANNER 🔍                 ║
║                                                               ║
║  Monitoring: Pump.fun, Raydium, Meteora                      ║
║  Detection: Volume spikes, buyer surges, early signals       ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝

14:23:15 🔥 HOT | 18.45 SOL | 12 buyers | Age: 8.3 min | Score: 3.42 | Mint: 3adf...xyz
    └─ Recent 5min: 12.30 SOL from 8 buyers

14:24:02 🚀 BLAZING | 67.89 SOL | 24 buyers | Age: 15.1 min | Score: 8.91 | Mint: 9bcd...abc
    └─ Recent 5min: 23.45 SOL from 15 buyers
    ═══════════════════════════════════════════════════════════
```

## 🏗️ Architecture

### Components

1. **Block Fetcher** (`src/block_fetcher.rs`)
   - Connects to Solana RPC
   - Polls for latest slots and fetches block data
   - Handles network errors and skipped slots

2. **Transaction Parser** (`src/transaction_parser.rs`)
   - Filters transactions by program ID (Pump.fun, Raydium, Meteora)
   - Extracts token events (mints, swaps, buys)
   - Calculates SOL amounts from balance changes

3. **Token Tracker** (`src/token_tracker.rs`)
   - Maintains in-memory stats for each token
   - Tracks volume, unique buyers, and timestamps
   - Provides methods for recent activity analysis

4. **Spike Detector** (`src/spike_detector.rs`)
   - Evaluates tokens against configurable thresholds
   - Calculates composite scores based on multiple factors
   - Prevents alert spam with cooldown periods

5. **Console Output** (`src/console_output.rs`)
   - Beautiful formatted output with emojis
   - Real-time status updates
   - Multiple display formats (detailed/compact)

### Detection Criteria

**Minimum Thresholds:**
- 2+ SOL total volume
- 3+ unique buyers
- Token age < 1 hour

**Alert Levels:**
- **Warm 🌡️**: 5+ SOL or 5+ buyers
- **Hot 🔥**: 15+ SOL or 10+ buyers
- **Blazing 🚀**: 50+ SOL or 20+ buyers

**Scoring Factors:**
- Total volume (30% weight)
- Unique buyer count (30% weight)
- Token recency (20% weight)
- Recent activity surge (20% weight)

## ⚙️ Configuration

### Detection Thresholds

Modify `src/spike_detector.rs` to adjust detection sensitivity:

```rust
impl Default for SpikeThresholds {
    fn default() -> Self {
        Self {
            min_volume: 2.0,           // Minimum SOL volume
            min_buyers: 3,             // Minimum unique buyers
            max_age_minutes: 60.0,     // Maximum token age
            warm_volume: 5.0,          // Volume for "warm" alert
            hot_volume: 15.0,          // Volume for "hot" alert
            blazing_volume: 50.0,      // Volume for "blazing" alert
            // ... more thresholds
        }
    }
}
```

### RPC Endpoint

Change the RPC endpoint in `src/main.rs`:

```rust
let block_fetcher = BlockFetcher::new("https://your-rpc-endpoint.com")?;
```

## 🔬 Development

### Adding New DEX Support

1. Add program ID to `src/transaction_parser.rs`:
```rust
pub const NEW_DEX_PROGRAM_ID: &str = "YourProgramIdHere";
```

2. Update the parser to handle the new program:
```rust
impl TransactionParser {
    pub fn new() -> Self {
        Self {
            // ... existing programs
            new_dex_program: Pubkey::from_str(NEW_DEX_PROGRAM_ID).unwrap(),
        }
    }
}
```

### Custom Event Types

Add new event types in `src/transaction_parser.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    TokenMint,
    Swap,
    Buy,
    LpAction,
    YourNewEventType,  // Add here
}
```

## 🚀 Future Enhancements (Steps 6-8)

### Step 6: gRPC Stream Integration
- Replace RPC polling with real-time Yellowstone Geyser stream
- Significantly reduce latency and improve data freshness
- Handle both transaction and block confirmations

### Step 7: De-Duplication System  
- Implement LRU cache for transaction signatures
- Prevent double-processing of transactions seen in both live stream and blocks
- Optimize memory usage for signature tracking

### Step 8: Database Integration
- Add Postgres/MongoDB support via Prisma ORM
- Persist token metrics and historical data
- Enable API queries and frontend integration
- Support for microservices architecture

## 🐛 Troubleshooting

### Common Issues

1. **RPC Rate Limiting**
   - Use a premium RPC provider (Alchemy, QuickNode, etc.)
   - Increase polling interval in main loop

2. **High Memory Usage**
   - Reduce `max_tokens` in TokenTracker
   - Adjust cleanup thresholds

3. **No Hot Tokens Detected**
   - Lower detection thresholds in SpikeDetector
   - Check if program IDs are current
   - Verify RPC endpoint is working

### Debug Mode

Run with debug logging to see detailed parsing:
```bash
RUST_LOG=debug cargo run
```

## 📊 Performance

- **Memory Usage**: ~50-100MB for 10k tracked tokens
- **CPU Usage**: Low (~5-10% on modern hardware)  
- **Network**: ~1-2 MB/min RPC calls
- **Latency**: ~400ms polling interval (configurable)

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details.

## ⚠️ Disclaimer

This tool is for educational and research purposes. Always verify token information through multiple sources before making any financial decisions. 