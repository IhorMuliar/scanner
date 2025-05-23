# Project Context

You're contributing to a new web3 project (similar to Photon) at MVP stage. Your test task is to create a blockchain scanner that mimics MEVX’s Degen Zone, but using RPC calls only (no frontend scraping). The scanner should:

Monitor tokens launched/traded on Pump.fun, Raydium, and Meteora.

Detect volume or trading spikes.

Identify early pump signals.

Run efficiently with high responsiveness.

🎯 Your POC (Proof of Concept) Goal
Build a single-service app (no microservices yet) that:

Connects to Solana mainnet RPC.

Tracks real-time token activity from Pump.fun, Raydium, and Meteora.

Detects spikes in activity (e.g., unusual volume/buy counts).

Logs hot tokens and their stats to the console.

Skips DB or UI for now – just log data to demonstrate logic & results.

🧠 How It Works (Architecture Summary)
Key Flow:
Connect to Solana RPC using getSlot() and getBlock() with @solana/web3.js.

Scan parsed instructions in each transaction for activity related to:

Pump.fun smart contract ID

Raydium AMM instructions

Meteora swap or liquidity events

Track token metrics in-memory:

Token mint address

Total volume (in SOL)

Unique buyers

First seen / last seen timestamps

Every 5–15 seconds, evaluate token stats for spike conditions:

Volume > X SOL

≥ Y unique buyers

Token age < Z mins

If spike detected, console.log() with clear format:

yaml
Copy
Edit
[HOT] $PEPE — Volume: 15.2 SOL | Buyers: 8 | Age: 6 min
🔍 Spike Detection Heuristic (Simple Initial Rule)
ts
Copy
Edit
if (volume > 10 && buyers > 5 && age < 15 minutes) {
  logTokenAsHot()
}
You can improve this later with rolling averages or Z-score-based signals.

⚙️ Development Stack (Flexible)
Runtime: Node.js (easiest to prototype) or Rust if desired later

Tools:

@solana/web3.js

getBlock, getParsedTransaction

No database, no frontend — just console logging

Optional future extension: add SQLite, REST API, or dashboard UI

📦 Deliverable (for now)
A single script or service that:

Streams blocks from Solana

Detects new/active tokens from specified platforms

Tracks their volume and activity

Outputs a live list of “pumping” tokens to the console

🧠 Ready for Claude Implementation
Claude can now:

Help implement the scanner loop

Decode and filter instructions by program IDs

Track token stats and spike signals

Format console logs for clarity

