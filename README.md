# Solana Token Scanner (Rust)

A real-time Solana blockchain scanner that monitors token activity on Pump.fun, Raydium, and Meteora platforms. This Rust implementation detects early signals of "degen" behavior such as rapid buys, high SOL inflows, and sudden volume spikes.

## 🚀 Features

- **Real-time Block Scanning**: Continuously monitors Solana blockchain for new blocks
- **Multi-Platform Support**: Tracks activity on Pump.fun, Raydium, and Meteora
- **Spike Detection**: Identifies tokens with high volume and buyer activity
- **Memory Efficient**: Automatic cleanup of old token data
- **Configurable Thresholds**: Customizable volume, buyer count, and age limits
- **Structured Logging**: Comprehensive logging with configurable levels

## 🛠️ Tech Stack

- **Language**: Rust 🦀
- **Async Runtime**: Tokio
- **Solana Client**: solana-client, solana-sdk
- **Concurrency**: DashMap for thread-safe collections
- **CLI**: Clap for command-line interface
- **Logging**: env_logger with log crate

## 📁 Project Structure

```
src/
├── main.rs          # Main entry point with CLI
├── lib.rs           # Library module exports
├── config.rs        # Configuration structures
├── scanner.rs       # Core scanner logic
└── utils.rs         # Utility functions and types
Cargo.toml           # Project dependencies
README.md            # This file
```

## 🔧 Installation & Setup

### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Solana RPC access (mainnet-beta recommended)

### Build & Run

```bash
# Clone the repository
git clone <your-repo-url>
cd solana-token-scanner

# Build the project
cargo build --release

# Run with default settings
cargo run --release

# Or run with custom parameters
cargo run --release -- \
    --rpc-url "https://api.mainnet-beta.solana.com" \
    --volume-threshold 5.0 \
    --buyers-threshold 3 \
    --age-threshold 10
```

## ⚙️ Configuration

### Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `--rpc-url` | `https://api.mainnet-beta.solana.com` | Solana RPC endpoint |
| `--scan-interval` | `10000` | Scan interval in milliseconds |
| `--volume-threshold` | `1.0` | Minimum volume in SOL for spike detection |
| `--buyers-threshold` | `1` | Minimum unique buyers for spike detection |
| `--age-threshold` | `15` | Maximum token age in minutes |
| `--max-blocks` | `10` | Maximum blocks to process per batch |

### Environment Variables

Set `RUST_LOG` to control logging level:

```bash
# Info level (default)
RUST_LOG=info cargo run

# Debug level for more detailed output
RUST_LOG=debug cargo run

# Error level for minimal output
RUST_LOG=error cargo run
```

## 🎯 Detection Logic

The scanner identifies "hot" tokens based on:

1. **Volume Threshold**: Total SOL volume exceeds configured minimum
2. **Buyer Diversity**: Number of unique buyers meets minimum requirement
3. **Recency**: Token age is within the specified time window
4. **Platform Activity**: Activity detected on monitored DEX platforms

### Example Output

```
🔥 [HOT] $3adf — Volume: 14.20 SOL | Buyers: 7 | Age: 6 min | Platform: Pump.fun
🔥 [HOT] $b2c9 — Volume: 8.50 SOL | Buyers: 4 | Age: 12 min | Platform: Raydium
```

## 🏗️ Architecture

### Core Components

1. **TokenScanner**: Main scanner orchestrator
2. **Config**: Configuration management
3. **TokenMetrics**: In-memory token tracking
4. **ProgramIds**: Platform program ID management
5. **HotToken**: Spike detection results

### Data Flow

```
RPC Polling → Block Processing → Transaction Analysis → Token Metrics → Spike Detection → Console Output
```

### Concurrency Model

- **Thread-safe Collections**: DashMap for concurrent token metrics
- **Async Processing**: Tokio for non-blocking I/O
- **Atomic Operations**: For shared state management
- **Signal Handling**: Graceful shutdown on SIGINT/SIGTERM

## 🔮 Future Enhancements (Steps 6-8)

### Step 6: gRPC Stream Integration
- Replace RPC polling with real-time Yellowstone Geyser stream
- Implement `subscribe_with_request()` for live transactions
- Enhanced performance and lower latency

### Step 7: De-duplication System
- LRU cache for transaction signatures
- Avoid double-processing between live stream and confirmed blocks
- Memory-efficient signature tracking

### Step 8: Database Integration
- Postgres/MongoDB integration via Prisma
- Persistent token metadata storage
- API-ready data for frontend consumption
- Microservices architecture support

## 🐛 Error Handling

The scanner implements robust error handling:

- **Connection Failures**: Automatic retry with exponential backoff
- **Block Processing Errors**: Skip problematic blocks and continue
- **Transaction Parsing**: Graceful handling of malformed data
- **Memory Management**: Automatic cleanup prevents memory leaks

## 🔧 Development

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Documentation

```bash
cargo doc --open
```

## 📊 Performance

- **Memory Usage**: ~10-50MB depending on token activity
- **CPU Usage**: Low, primarily I/O bound
- **Network**: RPC call frequency based on scan interval
- **Throughput**: Processes 10+ blocks per scan cycle

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

This tool is for educational and research purposes. Token trading involves significant risk. Always do your own research and never invest more than you can afford to lose.
