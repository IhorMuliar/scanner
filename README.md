# Solana Token Scanner

A blockchain scanner that monitors tokens launched/traded on Pump.fun, Raydium, and Meteora, detecting volume/trading spikes and identifying early pump signals.

## Features

- Connects to Solana mainnet RPC using @solana/web3.js
- Tracks real-time token activity from Pump.fun, Raydium, and Meteora
- Detects spikes in activity (unusual volume/buy counts)
- Logs hot tokens and their stats to the console
- Uses configurable thresholds for spike detection

## Requirements

- Node.js v16 or higher
- npm or yarn

## Installation

1. Clone the repository:

```bash
git clone https://github.com/yourusername/solana-token-scanner.git
cd solana-token-scanner
```

2. Install dependencies:

```bash
npm install
```

## Usage

Start the scanner:

```bash
npm start
```

The scanner will connect to Solana mainnet and start monitoring transactions for token activity, logging any detected "hot" tokens to the console.

## Configuration

You can modify the scanner behavior by editing the CONFIG object in `index.js`:

- `SOLANA_RPC_URL`: URL of the Solana RPC endpoint to connect to
- `SCAN_INTERVAL_MS`: Interval in milliseconds between scanning cycles
- `VOLUME_THRESHOLD`: Minimum volume in SOL required for a spike
- `BUYERS_THRESHOLD`: Minimum number of unique buyers required for a spike
- `AGE_THRESHOLD_MINUTES`: Maximum age in minutes for a token to be considered new

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

## Notes

- This is a POC implementation with simplified logic. In a production environment, we would want to add more robust error handling, logging, and monitoring.
- For accurate token identification and detailed instruction parsing, we would need to implement program-specific instruction decoders.
- The token extraction logic are simplified and would need to be updated with actual values and more detailed parsing.
