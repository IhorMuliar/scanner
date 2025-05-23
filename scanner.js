import { Connection } from '@solana/web3.js';
import { 
  PROGRAM_IDS,
  formatSol,
  getTokenAge,
  generateSymbol,
  determinePlatform,
  createTokenMetrics,
  sleep
} from './utils.js';
import { CONFIG, CONNECTION_CONFIG, BLOCK_CONFIG, OUTPUT_FORMAT } from './config.js';

/**
 * Solana Token Scanner
 * Monitors transactions on Solana for token trading activity and detects spikes
 */
export const TokenScanner = {
  connection: null,
  isScanning: false,
  tokenMetrics: new Map(),
  lastProcessedSlot: 0,
  
  /**
   * Initializes the scanner by connecting to Solana RPC
   */
  async initialize() {
    console.log('🚀 Initializing Solana Token Scanner...');
    this.connection = new Connection(CONFIG.SOLANA_RPC_URL, CONNECTION_CONFIG);
    console.log(`✅ Connected to Solana RPC: ${CONFIG.SOLANA_RPC_URL}`);
    
    // Get current slot to start scanning from
    this.lastProcessedSlot = await this.connection.getSlot();
    console.log(`✅ Starting scan from slot: ${this.lastProcessedSlot}`);
    
    console.log(`📊 Spike detection criteria:`);
    console.log(`   - Volume > ${CONFIG.VOLUME_THRESHOLD} SOL`);
    console.log(`   - Unique buyers ≥ ${CONFIG.BUYERS_THRESHOLD}`);
    console.log(`   - Token age < ${CONFIG.AGE_THRESHOLD_MINUTES} minutes`);
    console.log('\n🔍 Scanning for token activity...');
    
    return this;
  },

  /**
   * Starts the continuous scanning loop
   */
  async startScanning() {
    if (this.isScanning) return;
    this.isScanning = true;
    
    while (this.isScanning) {
      try {
        await this.scanNewBlocks();
        await this.detectSpikes();
        await sleep(CONFIG.SCAN_INTERVAL_MS);
      } catch (error) {
        console.error(`❌ Error during scan: ${error.message}`);
        // Continue scanning despite errors
      }
    }
  },

  /**
   * Scans new blocks since the last processed slot
   */
  async scanNewBlocks() {
    // Get the latest slot
    const currentSlot = await this.connection.getSlot();
    
    if (currentSlot <= this.lastProcessedSlot) {
      return; // No new blocks to process
    }
    
    // Process blocks in batches to avoid overwhelming the RPC
    const maxBlocksToProcess = CONFIG.MAX_BLOCKS_TO_PROCESS;
    const endSlot = Math.min(currentSlot, this.lastProcessedSlot + maxBlocksToProcess);
    
    for (let slot = this.lastProcessedSlot + 1; slot <= endSlot; slot++) {
      await this.processBlock(slot);
    }
    
    this.lastProcessedSlot = endSlot;
  },

  /**
   * Processes a single block, analyzing its transactions
   * @param {number} slot - The block slot to process
   */
  async processBlock(slot) {
    try {
      // Get block with transaction details
      const block = await this.connection.getBlock(slot, BLOCK_CONFIG);
      
      if (!block || !block.transactions) return;
      
      for (const tx of block.transactions) {
        await this.processTransaction(tx, block.blockTime);
      }
    } catch (error) {
      // Don't crash on block fetch errors, just log and continue
      console.error(`❌ Error processing block ${slot}: ${error.message}`);
    }
  },

  /**
   * Processes a single transaction, looking for token activity
   * @param {Object} tx - Transaction object from Solana web3.js
   * @param {number} blockTime - Block timestamp
   */
  async processTransaction(tx, blockTime) {
    if (!tx.meta || tx.meta.err || !tx.transaction.message) return;
    
    try {
      const message = tx.transaction.message;
      
      // Check if transaction involves any of our target programs
      if (!message.accountKeys) return;

      const accountKeys = message.accountKeys.map(key => key.toString ? key.toString() : key);
      const programIds = [
        PROGRAM_IDS.PUMP_FUN.toString(),
        PROGRAM_IDS.RAYDIUM_AMM.toString(),
        PROGRAM_IDS.METEORA.toString()
      ];
      
      // Check if any program IDs are involved in the transaction
      const relevantProgramIndex = accountKeys.findIndex(key => programIds.includes(key));
      if (relevantProgramIndex === -1) return;
      
      // Extract token information from the transaction
      const relevantToken = this.extractTokenInfo(tx, relevantProgramIndex);
      if (!relevantToken) return;
      
      // Update token metrics
      this.updateTokenMetrics(relevantToken, tx, blockTime);
    } catch (error) {
      console.error(`❌ Error processing transaction: ${error.message}`);
    }
  },

  /**
   * Extracts token information from a transaction
   * @param {Object} tx - Transaction object
   * @param {number} programIndex - Index of the relevant program in account keys
   * @returns {Object|null} Token information or null if not found
   */
  extractTokenInfo(tx, programIndex) {
    // This is a simplified implementation
    // In a real-world scenario, you'd need to parse the instruction data 
    // based on the specific program's instruction format
    
    try {
      // This is placeholder logic - actual implementation would depend on the specific programs
      const message = tx.transaction.message;
      const programId = message.accountKeys[programIndex].toString();
      
      // Find token mint address - this is a placeholder approach
      // Real implementation would need to decode instructions based on the specific program
      let tokenMintAddress = null;
      
      // For demonstration purposes only - real implementation would be more complex
      if (tx.meta && tx.meta.postTokenBalances && tx.meta.postTokenBalances.length > 0) {
        const tokenBalance = tx.meta.postTokenBalances.find(balance => 
          balance.uiTokenAmount && balance.uiTokenAmount.uiAmount > 0
        );
        
        if (tokenBalance) {
          tokenMintAddress = tokenBalance.mint;
        }
      }
      
      if (!tokenMintAddress) return null;
      
      // Extract volume (in lamports)
      let volumeLamports = 0;
      if (tx.meta && tx.meta.preBalances && tx.meta.postBalances) {
        // Simple estimation of SOL transfer amount
        for (let i = 0; i < tx.meta.preBalances.length; i++) {
          const preBalance = tx.meta.preBalances[i];
          const postBalance = tx.meta.postBalances[i];
          if (preBalance > postBalance) {
            volumeLamports += preBalance - postBalance;
          }
        }
      }
      
      // Get buyer address (simplified)
      const buyerAddress = tx.transaction.message.accountKeys[0].toString();
      
      return {
        tokenMint: tokenMintAddress,
        volume: volumeLamports,
        buyer: buyerAddress,
        platform: determinePlatform(programId)
      };
    } catch (error) {
      console.error(`❌ Error extracting token info: ${error.message}`);
      return null;
    }
  },

  /**
   * Updates the in-memory token metrics map with new token information
   * @param {Object} tokenInfo - Token information extracted from transaction
   * @param {Object} tx - Transaction object
   * @param {number} blockTime - Block timestamp
   */
  updateTokenMetrics(tokenInfo, tx, blockTime) {
    const { tokenMint, volume, buyer, platform } = tokenInfo;
    const timestamp = blockTime ? blockTime * 1000 : Date.now();
    
    if (!this.tokenMetrics.has(tokenMint)) {
      // New token discovered
      this.tokenMetrics.set(
        tokenMint, 
        createTokenMetrics({ tokenMint, volume, buyer, platform, timestamp })
      );
    } else {
      // Update existing token metrics
      const metrics = this.tokenMetrics.get(tokenMint);
      metrics.totalVolume += volume;
      metrics.uniqueBuyers.add(buyer);
      metrics.lastSeen = timestamp;
      metrics.transactions += 1;
    }
  },

  /**
   * Detects spikes in token activity and logs them to console
   */
  async detectSpikes() {
    const now = Date.now();
    const hotTokens = [];
    
    this.tokenMetrics.forEach((metrics, mintAddress) => {
      // Skip tokens older than threshold
      const ageMinutes = (now - metrics.firstSeen) / (60 * 1000);
      if (ageMinutes > CONFIG.AGE_THRESHOLD_MINUTES) return;
      
      const volumeInSol = metrics.totalVolume / 1e9;
      const uniqueBuyersCount = metrics.uniqueBuyers.size;
      
      // Apply spike detection criteria
      if (volumeInSol > CONFIG.VOLUME_THRESHOLD && uniqueBuyersCount >= CONFIG.BUYERS_THRESHOLD) {
        hotTokens.push({
          ...metrics,
          volumeInSol,
          uniqueBuyersCount,
          ageMinutes: Math.floor(ageMinutes)
        });
      }
    });
    
    // Sort by volume and log the results
    hotTokens
      .sort((a, b) => b.volumeInSol - a.volumeInSol)
      .forEach(token => this.logHotToken(token));
    
    // Clean up old tokens from memory to prevent memory leaks
    this.cleanupOldTokens();
  },

  /**
   * Logs information about a hot token to the console
   * @param {Object} token - Hot token metrics
   */
  logHotToken(token) {
    console.log("⛓️", token);
    const { HOT_TOKEN_PREFIX, SEPARATOR, VOLUME_LABEL, BUYERS_LABEL, AGE_LABEL, PLATFORM_LABEL, AGE_UNIT } = OUTPUT_FORMAT;
    
    console.log(
      `${HOT_TOKEN_PREFIX} ${token.symbol} — ` + 
      `${VOLUME_LABEL}${token.volumeInSol.toFixed(2)} SOL${SEPARATOR}` +
      `${BUYERS_LABEL}${token.uniqueBuyersCount}${SEPARATOR}` + 
      `${AGE_LABEL}${token.ageMinutes}${AGE_UNIT}${SEPARATOR}` + 
      `${PLATFORM_LABEL}${token.platform}`
    );
  },

  /**
   * Removes old tokens from memory to prevent memory leaks
   */
  cleanupOldTokens() {
    const now = Date.now();
    const oldThresholdMs = CONFIG.AGE_THRESHOLD_MINUTES * 60 * 1000 * CONFIG.CLEANUP_AGE_MULTIPLIER;
    
    this.tokenMetrics.forEach((metrics, mintAddress) => {
      if (now - metrics.lastSeen > oldThresholdMs) {
        this.tokenMetrics.delete(mintAddress);
      }
    });
  },

  /**
   * Stops the scanner
   */
  stop() {
    this.isScanning = false;
    console.log('🛑 Scanner stopped.');
  }
}; 