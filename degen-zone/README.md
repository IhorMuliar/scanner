# Solana Transaction Scanner

A high-performance Rust application for scanning the Solana blockchain in real-time. This tool monitors blockchain activity, processes blocks sequentially, and provides detailed logging of each individual transaction as it's discovered.

## Features

- 🚀 **Real-time Transaction Scanning**: Continuously monitors the Solana blockchain for new transactions
- 📊 **Comprehensive Transaction Logging**: Detailed information for each transaction including signature, status, fees, compute units, and accounts
- ⚡ **High Performance**: Built with Rust and async/await for optimal performance
- 🔧 **Configurable**: Command-line arguments and environment variable support
- 🛡️ **Error Handling**: Robust error handling with automatic retry mechanisms
- 💾 **Deduplication**: Smart caching to avoid processing the same block twice
- 🎯 **Flexible Starting Point**: Start from latest block or specific slot number
- 🔄 **Graceful Shutdown**: Clean shutdown on Ctrl+C with statistics summary
- ✅ **Success/Failure Detection**: Clear indication of successful vs failed transactions

## Prerequisites

- Rust 1.70+ installed
- Internet connection to access Solana RPC endpoints

## Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd scanner
   ```

2. Build the application:
   ```bash
   cargo build --release
   ```

## Usage

### Basic Usage

Run the scanner with default settings (mainnet, 1-second intervals):
```bash
cargo run
```

### Command Line Options

```bash
cargo run -- --help
```

Available options:
- `-r, --rpc-url <URL>`: Solana RPC endpoint (default: mainnet-beta)
- `-i, --interval <SECONDS>`: Polling interval in seconds (default: 1)
- `-s, --start-slot <SLOT>`: Starting slot number (default: 0 for latest)
- `-m, --max-blocks <COUNT>`: Maximum blocks to process (default: 0 for unlimited)

### Examples

**Scan mainnet starting from latest block:**
```bash
cargo run -- --rpc-url https://api.mainnet-beta.solana.com
```

**Scan testnet with 2-second intervals:**
```bash
cargo run -- --rpc-url https://api.testnet.solana.com --interval 2
```

**Process only 5 blocks starting from a specific slot:**
```bash
cargo run -- --start-slot 250000000 --max-blocks 5
```

**Development with debug logging:**
```bash
RUST_LOG=debug cargo run
```

## Output Format

Each transaction will be logged with the following information:

```
💳 NEW TRANSACTION DETECTED
  ✅ Status: SUCCESS
  📝 Signature: 5GGa6C3YUdKeshbrbf2vw1XjuytJ1kRDg7hBdBy3uMfe5DPqHAdcaKZhfh5xLMsTV5cX9NPDQQzFLGJvK67AQxUC
  📍 Slot: 336361683
  🔗 Block: LcsPEnF6Z3eDyAUkrTmvXUBKLh8E1LNJi7Ek2qyuV22
  🧾 Instructions: 1
  💰 Fee: 0.000005 SOL (5000 lamports)
  ⚡ Compute Units: 2100
  👥 Accounts: 3
  🔑 Recent Blockhash: 5WksNRxXBcXX1V6MFdyZ9tBqYZPwhs...
  ⏰ Processed At: 12:43:48.698
  ➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖➖
```

For failed transactions, you'll also see:
```
  ❌ Status: FAILED
  ⚠️  Error: InstructionError(0, Custom(0))
```

## Configuration

### Environment Variables

Create a `.env` file in the project root to set default configuration:

```env
# Solana RPC endpoint
RPC_URL=https://api.mainnet-beta.solana.com

# Logging level
RUST_LOG=info
```

### Logging Levels

- `error`: Only critical errors
- `warn`: Warnings and errors
- `info`: General information (default)
- `debug`: Detailed debugging information
- `trace`: Very verbose logging

## Network Options

### Mainnet
```bash
cargo run -- --rpc-url https://api.mainnet-beta.solana.com
```

### Testnet
```bash
cargo run -- --rpc-url https://api.testnet.solana.com
```

### Devnet
```bash
cargo run -- --rpc-url https://api.devnet.solana.com
```

### Custom RPC
```bash
cargo run -- --rpc-url https://your-custom-rpc.com
```

## Performance Tips

1. **Adjust Polling Interval**: Lower intervals (0.5s) for faster scanning, higher intervals (2-5s) for less load
2. **Use Custom RPC**: For better performance, consider using a dedicated RPC endpoint
3. **Resource Monitoring**: Monitor CPU and memory usage during long-running scans
4. **Network Stability**: Ensure stable internet connection for consistent scanning

Note: Transaction volumes can be very high (thousands per block), so consider starting with testnet or limited block counts for testing.

## Troubleshooting

### Common Issues

**Connection Errors:**
- Verify RPC endpoint is accessible
- Check internet connection
- Try different RPC endpoints

**Rate Limiting:**
- Increase polling interval with `--interval`
- Use a dedicated RPC service
- Consider implementing request throttling

**High Output Volume:**
- Transactions can be very numerous (5000+ per block on mainnet)
- Use `--max-blocks` to limit output for testing
- Consider output redirection: `cargo run > transactions.log`

### Debug Mode

Enable debug logging for troubleshooting:
```bash
RUST_LOG=debug cargo run
```

## Architecture

The scanner consists of several key components:

- **SolanaBlockScanner**: Main scanner struct that manages the scanning process
- **ProcessedTransaction**: Data structure representing a processed transaction with detailed metadata
- **ProcessedBlock**: Data structure representing a processed block with metadata
- **RPC Client**: Handles communication with Solana blockchain
- **Caching System**: Prevents duplicate processing using DashMap
- **Error Handling**: Comprehensive error handling with retry logic

## Transaction Data

Each transaction log includes:

- **Signature**: Unique transaction identifier
- **Status**: SUCCESS or FAILED with error details
- **Slot & Block**: Location information on the blockchain
- **Instructions**: Number of instructions in the transaction
- **Fee**: Transaction fee in SOL and lamports
- **Compute Units**: Computational resources consumed
- **Accounts**: Number of accounts involved
- **Recent Blockhash**: Blockhash used for the transaction
- **Timestamp**: When the transaction was processed

## Future Enhancements

Current version provides comprehensive transaction logging. Future versions could include:
- Transaction filtering by program/account
- Token transfer detection and analysis
- Smart contract interaction monitoring
- Database storage options
- Real-time alerting for specific transaction patterns
- JSON output format for integration

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. 