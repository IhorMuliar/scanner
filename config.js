/**
 * Configuration for the Solana Token Scanner
 */

/**
 * Scanner configuration settings
 */
export const CONFIG = {
  // Solana RPC endpoint URL
  SOLANA_RPC_URL: 'https://api.mainnet-beta.solana.com',
  
  // Scanning parameters
  SCAN_INTERVAL_MS: 10000, // 10 seconds between scan cycles
  MAX_BLOCKS_TO_PROCESS: 10, // Maximum blocks to process in one batch
  
  // Spike detection thresholds
  VOLUME_THRESHOLD: 1, // Minimum volume in SOL
  BUYERS_THRESHOLD: 1, // Minimum unique buyers
  AGE_THRESHOLD_MINUTES: 15, // Maximum age to be considered "new"
  
  // Memory management
  CLEANUP_INTERVAL_MS: 60000, // How often to clean up old tokens (1 minute)
  CLEANUP_AGE_MULTIPLIER: 2, // Multiple of AGE_THRESHOLD to keep tokens in memory
};

/**
 * Connection parameters for Solana web3
 */
export const CONNECTION_CONFIG = {
  commitment: 'confirmed',
  disableRetryOnRateLimit: false,
  confirmTransactionInitialTimeout: 60000
};

/**
 * Block fetch configuration
 */
export const BLOCK_CONFIG = {
  maxSupportedTransactionVersion: 0,
  rewards: false,
  commitment: 'confirmed',
  transactionDetails: 'full'
};

/**
 * Console output formatting
 */
export const OUTPUT_FORMAT = {
  HOT_TOKEN_PREFIX: '[HOT]',
  SEPARATOR: ' | ',
  VOLUME_LABEL: 'Volume: ',
  BUYERS_LABEL: 'Buyers: ',
  AGE_LABEL: 'Age: ',
  PLATFORM_LABEL: 'Platform: ',
  AGE_UNIT: ' min'
}; 