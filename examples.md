# Solana Token Scanner Examples

This document provides common usage examples for the Solana blockchain scanner.

## Basic Usage Examples

### 1. Scan Latest Blocks on Mainnet
```bash
# Scan mainnet indefinitely starting from the latest block
cargo run

# Or explicitly specify mainnet
cargo run -- --rpc-url https://api.mainnet-beta.solana.com
```

### 2. Test with Limited Block Count
```bash
# Scan only 10 blocks for testing
cargo run -- --max-blocks 10

# Scan 5 blocks on testnet
cargo run -- --rpc-url https://api.testnet.solana.com --max-blocks 5
```

### 3. Different Network Environments

#### Testnet
```bash
cargo run -- --rpc-url https://api.testnet.solana.com
```

#### Devnet
```bash
cargo run -- --rpc-url https://api.devnet.solana.com
```

#### Custom RPC Endpoint
```bash
cargo run -- --rpc-url https://your-custom-rpc-endpoint.com
```

### 4. Adjust Polling Frequency

#### Fast Scanning (500ms intervals)
```bash
# Warning: Higher CPU/Network usage
cargo run -- --interval 0.5
```

#### Conservative Scanning (5 second intervals)
```bash
# Lower resource usage, good for long-term monitoring
cargo run -- --interval 5
```

### 5. Historical Block Scanning
```bash
# Start from a specific slot number
cargo run -- --start-slot 250000000 --max-blocks 100

# Scan 1000 blocks starting from slot 300000000
cargo run -- --start-slot 300000000 --max-blocks 1000
```

### 6. Debug and Development

#### Enable Debug Logging
```bash
# Verbose output for troubleshooting
RUST_LOG=debug cargo run -- --max-blocks 3
```

#### Trace Level Logging
```bash
# Very detailed logging (use sparingly)
RUST_LOG=trace cargo run -- --max-blocks 1
```

### 7. Production Monitoring

#### Long-term Monitoring with Error Handling
```bash
# Production setup with reasonable polling
cargo run -- --interval 2 > scanner.log 2>&1 &
```

#### Monitor Specific Block Range
```bash
# Monitor blocks from slot 250M to 250M+10K
cargo run -- --start-slot 250000000 --max-blocks 10000
```

## Environment Variables

Create a `.env` file for default configuration:

```env
# Default RPC endpoint
RPC_URL=https://api.mainnet-beta.solana.com

# Logging level
RUST_LOG=info

# Default polling interval (can be overridden by CLI)
POLLING_INTERVAL=1
```

## Performance Optimization Examples

### High-Frequency Scanning
```bash
# For real-time applications requiring fast updates
cargo run -- --interval 0.2 --rpc-url https://your-high-performance-rpc.com
```

### Resource-Efficient Scanning
```bash
# For systems with limited resources
cargo run -- --interval 10 --max-blocks 1000
```

## Output Filtering

While the current version logs all blocks, you can combine with shell tools:

### Save Output to File
```bash
cargo run -- --max-blocks 100 > blocks.log 2>&1
```

### Filter for Specific Information
```bash
cargo run -- --max-blocks 10 | grep "📍 Slot"
```

### Monitor Only Transaction Counts
```bash
cargo run -- --max-blocks 50 | grep "📊 Transactions"
```

## Automation Examples

### Continuous Monitoring Script
```bash
#!/bin/bash
# restart_scanner.sh

while true; do
    echo "Starting scanner..."
    cargo run -- --max-blocks 1000
    echo "Scanner stopped, restarting in 10 seconds..."
    sleep 10
done
```

### Health Check Script
```bash
#!/bin/bash
# health_check.sh

timeout 30s cargo run -- --max-blocks 1 > /tmp/scanner_test.log 2>&1
if [ $? -eq 0 ]; then
    echo "Scanner is healthy"
    exit 0
else
    echo "Scanner health check failed"
    exit 1
fi
```

## Common Issues and Solutions

### Rate Limiting
If you encounter rate limiting:
```bash
# Increase polling interval
cargo run -- --interval 3

# Use a dedicated RPC service
cargo run -- --rpc-url wss://your-dedicated-rpc.com
```

### Memory Usage
For long-running scans:
```bash
# Restart periodically to clear cache
cargo run -- --max-blocks 10000
# Then restart the process
```

### Network Issues
For unreliable connections:
```bash
# Use longer intervals to reduce failed requests
cargo run -- --interval 5
```

## Next Steps

After verifying block scanning works correctly, you can:

1. **Add Transaction Processing**: Modify the code to process individual transactions within each block
2. **Add Filtering**: Implement filters for specific transaction types or accounts
3. **Add Database Storage**: Store block and transaction data in a database
4. **Add Real-time Alerts**: Implement notifications for specific blockchain events
5. **Add Web Interface**: Create a web dashboard to visualize the data

## Performance Benchmarks

On a typical development machine:
- **Mainnet**: Can process ~400 slots/second with 1s polling
- **Testnet**: Can process ~200 slots/second with 1s polling  
- **Memory**: ~50MB baseline, grows slowly with cache
- **CPU**: ~5-10% on modern hardware

## Troubleshooting

If you encounter issues:

1. Check your internet connection
2. Verify the RPC endpoint is accessible
3. Try a different Solana network (testnet/devnet)
4. Increase polling interval if getting rate limited
5. Enable debug logging: `RUST_LOG=debug cargo run`

For more help, check the README.md file or create an issue in the repository. 