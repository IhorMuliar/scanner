# Project Overview

Goal: Create a Rust application that monitors pump.fun tokens in real-time with phased implementation:

1. Log newly created tokens
2. Log graduated tokens
3. Log tokens about to graduate (future enhancement)

## Requirements

- Pure RPC transaction monitoring (no APIs)
- No database integration initially
- Real-time logging of token lifecycle events
- Monitor pump.fun program: `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`

## Understanding Bonding Curve State

### Bonding Curve Structure

Based on the provided IDL and state example, the bonding curve contains:

- **Virtual Token Reserves**: Total virtual token amount for pricing calculations
- **Virtual SOL Reserves**: Total virtual SOL amount for pricing calculations  
- **Real Token Reserves**: Actual tokens remaining for sale
- **Real SOL Reserves**: Actual SOL collected from sales
- **Token Total Supply**: Fixed total supply (typically 1,000,000,000,000,000)
- **Complete**: Boolean indicating if token has graduated
- **Creator**: Public key of token creator

## Implementation Priority

### Phase 1: Monitor New Token Creation

Focus on detecting and logging new tokens as they are created through the `create` instruction.

### Phase 2: Monitor Graduated Tokens

Track tokens that have completed graduation through the `migrate` instruction.

### Phase 3: Monitor About-to-Graduate Tokens (Future)

Implement more complex logic to predict which tokens are approaching graduation.

## Monitoring Strategy

### 1. Transaction-Based Monitoring (Primary Method)

**Approach**: Monitor specific instruction discriminators from pump.fun program
**Key Instructions**:

- **Create**: `[24, 30, 200, 40, 5, 28, 7, 119]` - New token creation
- **Migrate**: `[155, 234, 231, 146, 236, 158, 162, 30]` - Graduation to Raydium

### 2. Account-Based Monitoring (Secondary/Future Enhancement)

**Approach**: Subscribe to bonding curve account state changes
**Implementation** (for future phases):

- Subscribe to all existing bonding curve accounts
- Monitor new bonding curve account creation
- Parse account data changes to detect state transitions

## Event Detection Logic

### New Token Detection (Phase 1)

- Monitor for `create` instructions in transactions
- Extract mint address from accounts[0]
- Log initial token parameters and metadata
- No need to parse bonding curve state for basic detection

#### Graduated Token Detection (Phase 2)

- Monitor for `migrate` instructions in transactions
- Extract mint address from accounts[2]
- Extract pool information from accounts (not neccesary)
- Log graduation event with available metadata

#### About to Graduate Detection (Future Phase)

- Calculate graduation progress: `real_sol_reserves / 85_SOL_threshold`
- Set multiple warning levels: 50%, 70%, 80%, 90%, 95%, 99%
- Monitor buy velocity to estimate graduation timing
- Alert when tokens approach graduation threshold

## Technical Architecture

### Project Structure

```text
meme-zone/
├── src/
│   ├── main.rs                    # Application entry point
│   ├── monitor/
│   │   ├── mod.rs                 # Monitor module exports
│   │   ├── transaction_monitor.rs # Transaction instruction monitoring
│   │   └── event_detector.rs      # Event detection logic
│   ├── types/
│   │   ├── mod.rs                 # Type definitions
│   │   ├── instructions.rs        # Instruction definitions from IDL
│   │   └── events.rs              # Event structures for logging
│   └── utils/
│       ├── mod.rs                 # Utility functions
│       ├── rpc_client.rs          # RPC connection management
│       └── transaction_parser.rs  # Transaction parsing utilities
```

### Dependencies Required

Use dependencies from Cargo.toml

- **solana-client**: RPC connections and subscriptions
- **solana-sdk**: Transaction and account parsing
- **anchor-lang**: IDL integration and account deserialization
- **tokio**: Async runtime for concurrent monitoring
- **serde**: JSON serialization for logging
- **log/env_logger**: Structured logging framework
- **anyhow**: Error handling

## Implementation Phases

### Phase 1: New Token Creation Monitoring

**Goals**:

- Set up Rust project with required dependencies
- Establish RPC connection with retry logic
- Implement transaction monitoring for `create` instructions
- Extract and log new token creation events

### Phase 2: Graduated Token Monitoring

**Goals**:

- Extend transaction monitoring to include `migrate` instructions
- Implement parsing logic for graduation events
- Log token graduation with relevant metadata
- Connect creation and graduation events when possible

### Phase 3: Performance Optimization

**Goals**:

- Optimize subscription management
- Implement connection recovery
- Add rate limiting and error handling

### Future Phase: About-to-Graduate Monitoring

**Goals**:

- Implement bonding curve account monitoring
- Add graduation threshold detection logic
- Calculate and track graduation progress
- Implement multi-level warning system

## Event Logging Structure

### New Token Events (Phase 1)

**Fields to Log**:

- Timestamp of creation
- Transaction signature
- Mint address
- Creator public key
- Token metadata (if available)

### Graduated Token Events (Phase 2)

**Fields to Log**:

- Graduation timestamp
- Transaction signature
- Mint address
- Migration transaction hash
- Raydium pool address (if available)

### About to Graduate Events (Future Phase)

**Fields to Log**:

- Current timestamp
- Mint address
- Graduation progress percentage
- Current market cap in SOL
- SOL needed for graduation
- Estimated time to graduation (based on velocity)
- Recent buy volume metrics

## Monitoring Metrics

### Advanced Metrics (Future Phase)

**Current Price**: Based on bonding curve AMM formula
**Market Cap**: Current price × circulating supply
**Graduation Progress**: `real_sol_reserves / 85_SOL`
**Tokens Sold**: `total_supply - real_token_reserves`
**Buy Velocity**: SOL per minute based on recent transactions

## Risk Considerations

### Technical Risks

- RPC rate limiting and connection stability
- Network latency affecting real-time monitoring
- Solana network congestion during high activity
- Program updates changing IDL structure

### Monitoring Risks

- Missing events during connection drops
- Delayed event detection affecting accuracy
- High resource usage with many active tokens

## Success Metrics

### Performance Targets

- Event detection latency < 2 seconds
- 99.9% uptime for monitoring service
- Zero missed creation/graduation events

### Output Quality

- Structured JSON logging for easy parsing
- Comprehensive event metadata
- Clear event categorization
