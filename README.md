# Solana Token Scanner (Rust)

A real-time Solana blockchain scanner that monitors token activity on Pump.fun, Raydium, and Meteora platforms. This Rust implementation detects early signals of "degen" behavior such as rapid buys, high SOL inflows, and sudden volume spikes.

## 🚀 Features

- **Hybrid Architecture**: Combines RPC polling with real-time gRPC streaming (Yellowstone Geyser)
- **Resilient Fallback**: Automatically continues with RPC-only mode if gRPC connection fails
- **Real-time Block Scanning**: Continuously monitors Solana blockchain for new blocks
- **Multi-Platform Support**: Tracks activity on Pump.fun, Raydium, and Meteora
- **De-duplication System**: LRU cache prevents double-processing transactions
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

1. Clone the repository:

```bash
git clone https://github.com/yourusername/solana-token-scanner.git
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

2. Install dependencies:

```bash
npm install
```

## 🎯 Detection Logic

Start the scanner:

```bash
npm start
```

## 🏗️ Architecture

### Core Components

1. **TokenScanner**: Main scanner orchestrator
2. **Config**: Configuration management
3. **TokenMetrics**: In-memory token tracking
4. **ProgramIds**: Platform program ID management
5. **HotToken**: Spike detection results

### Data Flow

## How It Works

1. The scanner connects to the Solana mainnet RPC and starts monitoring new blocks.
2. For each block, it analyzes transactions looking for activity on the target platforms.
3. When relevant transactions are found, token metrics are tracked in memory.
4. Every scan interval, tokens are evaluated against the spike detection criteria.
5. Hot tokens meeting the criteria are logged to the console.

## Example Output

```bash
[HOT] $PEPE — Volume: 15.2 SOL | Buyers: 8 | Age: 6 min | Platform: Pump.fun
[HOT] $DOGE — Volume: 12.5 SOL | Buyers: 6 | Age: 10 min | Platform: Raydium
```

### Concurrency Model

- This is a POC implementation with simplified logic. In a production environment, we would want to add more robust error handling, logging, and monitoring.
- For accurate token identification and detailed instruction parsing, we would need to implement program-specific instruction decoders.
- The token extraction logic are simplified and would need to be updated with actual values and more detailed parsing.
