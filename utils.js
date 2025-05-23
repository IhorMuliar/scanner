import { PublicKey } from '@solana/web3.js';

/**
 * Utilities for the Solana Token Scanner
 */

/**
 * Formats lamports to SOL with 2 decimal places
 * @param {number} lamports - Amount in lamports
 * @returns {string} Formatted SOL amount
 */
export const formatSol = (lamports) => {
  const sol = lamports / 1e9;
  return `${sol.toFixed(2)} SOL`;
};

/**
 * Calculates token age in minutes
 * @param {number} firstSeen - Timestamp when token was first seen
 * @returns {number} Age in minutes
 */
export const getTokenAge = (firstSeen) => {
  const ageMs = Date.now() - firstSeen;
  return Math.floor(ageMs / (60 * 1000));
};

/**
 * Generates a simulated token symbol from the mint address
 * In a production app, you would query token metadata
 * @param {string} tokenMint - Token mint address
 * @returns {string} Generated symbol with $ prefix
 */
export const generateSymbol = (tokenMint) => {
  return `$${tokenMint.substring(0, 4)}`;
};

/**
 * Safely validates if string is a valid Solana public key
 * @param {string} address - Address to validate
 * @returns {boolean} True if valid
 */
export const isValidPublicKey = (address) => {
  try {
    new PublicKey(address);
    return true;
  } catch (error) {
    return false;
  }
};

/**
 * Known program IDs for platforms we're monitoring
 * Note: These are placeholder program IDs for demonstration purposes
 */
export const PROGRAM_IDS = {
  // In a production app, you would use the actual program IDs
  PUMP_FUN: new PublicKey('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'),
  RAYDIUM_AMM: new PublicKey('675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8'),
  METEORA: new PublicKey('LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo')
};

/**
 * Default configuration values for scanner
 */
export const DEFAULT_CONFIG = {
  SOLANA_RPC_URL: 'https://api.mainnet-beta.solana.com',
  SCAN_INTERVAL_MS: 10000, // 10 seconds
  VOLUME_THRESHOLD: 10, // SOL
  BUYERS_THRESHOLD: 5,
  AGE_THRESHOLD_MINUTES: 15,
  MAX_BLOCKS_TO_PROCESS: 10,
  CLEANUP_FACTOR: 2 // Multiple of age threshold to keep tokens in memory
};

/**
 * Handles sleep/delay in async functions
 * @param {number} ms - Milliseconds to sleep
 * @returns {Promise} Promise that resolves after ms
 */
export const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

/**
 * Determines platform name from program ID
 * @param {string} programId - Program ID string
 * @returns {string} Platform name
 */
export const determinePlatform = (programId) => {
  if (programId === PROGRAM_IDS.PUMP_FUN.toString()) return 'Pump.fun';
  if (programId === PROGRAM_IDS.RAYDIUM_AMM.toString()) return 'Raydium';
  if (programId === PROGRAM_IDS.METEORA.toString()) return 'Meteora';
  return 'Unknown';
};

/**
 * Creates an initial token metrics object
 * @param {Object} params - Parameters to initialize token metrics
 * @returns {Object} Token metrics object
 */
export const createTokenMetrics = ({ tokenMint, volume, buyer, platform, timestamp }) => {
  return {
    mint: tokenMint,
    symbol: generateSymbol(tokenMint),
    totalVolume: volume,
    uniqueBuyers: new Set([buyer]),
    firstSeen: timestamp,
    lastSeen: timestamp,
    platform,
    transactions: 1
  };
}; 