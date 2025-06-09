# Unified Meme Zone Token Scanner

A Rust-based token lifecycle monitor for various Solana meme token platforms.

## Overview

This scanner monitors Solana blockchain for transactions related to various meme token platforms:

- [Pump.fun](https://pump.fun/)
- [Raydium Launch Lab](https://raydium.io/launchpad/) (coming soon)
- [Moonit](https://moonit.so/) (coming soon)

The scanner tracks token creation, buys, sells, and other lifecycle events to provide real-time monitoring of token graduation progress and market statistics.

## Implementation Status

- ✅ Core trait definitions and interfaces created
- ✅ PumpFunStrategy implementation completed
- ✅ Scanner implementation using the strategy pattern
- ✅ Command line interface for selecting platforms
- 🚧 Testing and debugging in progress
- 🔄 Raydium and Moonit strategies to be implemented later

## Features

- **Platform Abstraction**: A unified codebase that can monitor multiple token platforms
- **Extensible Architecture**: Easy to add support for new platforms via the strategy pattern
- **Real-time Monitoring**: Processes Solana blocks as they're created
- **Graduation Tracking**: Monitors tokens' progress towards graduation
- **Comprehensive Logging**: Detailed information about token lifecycle events

## Architecture

The scanner uses a strategy pattern to abstract platform-specific behavior:

```
                              ┌────────────────────┐
                              │  SolanaBlockScanner │
                              └───────────┬────────┘
                                          │
                                          │ uses
                                          ▼
┌─────────────────────┐        ┌────────────────────┐
│ CommandLineInterface │───────▶│ TokenPlatformTrait │◀───── Strategy Pattern
└─────────────────────┘        └────────┬───────────┘
                                        │
                                        │ implemented by
                                        │
           ┌────────────────────────────┼────────────────────────────┐
           │                            │                            │
           ▼                            ▼                            ▼
┌────────────────────┐      ┌────────────────────┐      ┌────────────────────┐
│   PumpFunStrategy   │      │  RaydiumStrategy   │      │   MoonitStrategy   │
└────────────────────┘      └────────────────────┘      └────────────────────┘
```

## Installation

1. Ensure you have Rust and Cargo installed
2. Clone the repository
3. Build the project:

```bash
cd meme-zone
cargo build --release
```

## Usage

Run the scanner with default settings (Pump.fun platform):

```bash
cargo run --release
```

Specify a different RPC endpoint:

```bash
cargo run --release -- --rpc-url https://your-rpc-endpoint.com
```

Specify a different platform (currently only pump-fun is supported):

```bash
cargo run --release -- --platform pump-fun
```

Full options:

```bash
cargo run --release -- --help
```

## Configuration

The scanner accepts the following command-line arguments:

- `--rpc-url` (-r): Solana RPC endpoint URL
- `--platform` (-p): Platform to monitor (pump-fun, raydium, moonit)
- `--start-slot` (-s): Start slot for scanning (0 = latest)
- `--max-blocks` (-m): Maximum number of blocks to process
- `--interval-ms` (-i): Polling interval in milliseconds
- `--log-level` (-l): Log level (error, warn, info, debug, trace)

## Adding a New Platform

To add support for a new platform:

1. Create a new file with your platform implementation
2. Implement the `TokenPlatformTrait` trait
3. Create a concrete bonding curve state struct that implements `BondingCurveStateTrait`
4. Add the platform option to the command-line arguments in `main.rs`

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. 