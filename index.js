import { TokenScanner } from './scanner.js';

/**
 * Main entry point for the Solana Token Scanner
 */
const main = async () => {
  try {
    await TokenScanner.initialize();
    await TokenScanner.startScanning();
  } catch (error) {
    console.error(`❌ Fatal error: ${error.message}`);
    process.exit(1);
  }
};

// Handle shutdown gracefully
process.on('SIGINT', () => {
  console.log('\n⚠️ Received SIGINT. Shutting down gracefully...');
  TokenScanner.stop();
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\n⚠️ Received SIGTERM. Shutting down gracefully...');
  TokenScanner.stop();
  process.exit(0);
});

// Run the scanner
main(); 