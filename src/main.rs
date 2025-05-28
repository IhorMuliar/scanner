use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use bs58;
use chrono::{DateTime, Utc};
use clap::Parser;
use dashmap::DashMap;
use log::{debug, error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use tokio::time::{interval, sleep};

/// Pump.fun program ID - the main program responsible for token creation and trading
const PUMP_FUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

/// Instruction discriminators for Pump.fun program instructions
const BUY_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
const SELL_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
const CREATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];
const MIGRATE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [155, 234, 231, 146, 236, 158, 162, 30];
const SET_CREATOR_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [254, 148, 255, 112, 207, 142, 170, 165];

/// Solana Token Scanner - Monitor blockchain for new blocks and transactions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Solana RPC endpoint URL
    #[arg(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc_url: String,

    /// Polling interval in seconds
    #[arg(short, long, default_value_t = 1)]
    interval: u64,

    /// Starting slot number (0 for latest)
    #[arg(short, long, default_value_t = 0)]
    start_slot: u64,

    /// Maximum number of blocks to process (0 for unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_blocks: u64,
}

/// Represents a processed block with metadata
#[derive(Debug, Clone)]
struct ProcessedBlock {
    /// Slot number of the block
    slot: u64,
    /// Block hash
    blockhash: String,
    /// Parent slot
    parent_slot: u64,
    /// Block timestamp
    timestamp: Option<DateTime<Utc>>,
    /// Number of transactions in block
    transaction_count: usize,
    /// Processing timestamp
    processed_at: DateTime<Utc>,
}

/// Represents instruction type for a Pump.fun transaction
#[derive(Debug, Clone, PartialEq)]
enum PumpFunInstructionType {
    Buy,
    Sell,
    Create,
    Migrate,
    SetCreator,
    Other,
}

/// Represents a processed transaction with detailed information
#[derive(Debug, Clone)]
struct ProcessedTransaction {
    /// Transaction signature (unique identifier)
    signature: String,
    /// Slot number where transaction was included
    slot: u64,
    /// Block hash where transaction was included
    block_hash: String,
    /// Transaction success status
    is_successful: bool,
    /// Error message if transaction failed
    error_message: Option<String>,
    /// Number of instructions in the transaction
    instruction_count: usize,
    /// Compute units consumed by the transaction
    compute_units_consumed: Option<u64>,
    /// Fee paid for the transaction (in lamports)
    fee: u64,
    /// Accounts involved in the transaction
    account_keys: Vec<String>,
    /// Recent blockhash used by the transaction
    recent_blockhash: String,
    /// Whether this transaction involves Pump.fun program
    is_pump_fun_transaction: bool,
    /// Type of Pump.fun instruction if applicable
    pump_fun_instruction_type: Option<PumpFunInstructionType>,
    /// Processing timestamp
    processed_at: DateTime<Utc>,
}

/// Main scanner struct that manages the blockchain scanning process
pub struct SolanaBlockScanner {
    /// RPC client for connecting to Solana network (wrapped in Arc for sharing)
    rpc_client: Arc<RpcClient>,
    /// Cache to track processed blocks and avoid duplicates
    processed_blocks: DashMap<u64, ProcessedBlock>,
    /// Current slot being processed
    current_slot: u64,
    /// Configuration for polling interval
    polling_interval: Duration,
    /// Maximum blocks to process (0 for unlimited)
    max_blocks: u64,
    /// Counter for processed blocks
    blocks_processed: u64,
    /// Counter for processed transactions
    transactions_processed: u64,
}

impl SolanaBlockScanner {
    /// Create a new instance of the block scanner
    ///
    /// # Arguments
    /// * `rpc_url` - The Solana RPC endpoint URL
    /// * `start_slot` - Starting slot number (0 for latest)
    /// * `polling_interval` - How often to poll for new blocks
    /// * `max_blocks` - Maximum number of blocks to process
    ///
    /// # Returns
    /// * `Result<Self>` - New scanner instance or error
    pub async fn new(
        rpc_url: String,
        start_slot: u64,
        polling_interval: Duration,
        max_blocks: u64,
    ) -> Result<Self> {
        info!("🚀 Initializing Solana Transaction Scanner");
        info!("📡 Connecting to RPC endpoint: {}", rpc_url);

        // Initialize RPC client with confirmed commitment for reliability
        let rpc_client = Arc::new(RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed()));

        // Test connection by getting current slot (run in blocking task to avoid blocking async runtime)
        let latest_slot = tokio::task::spawn_blocking({
            let client = Arc::clone(&rpc_client);
            move || client.get_slot()
        })
        .await
        .context("Task join error")?
        .context("Failed to connect to Solana RPC endpoint")?;

        info!("✅ Successfully connected to Solana network");
        info!("🔢 Latest confirmed slot: {}", latest_slot);

        // Determine starting slot - use latest if 0 provided
        let current_slot = if start_slot == 0 {
            latest_slot
        } else {
            start_slot
        };

        info!("🎯 Starting scan from slot: {}", current_slot);

        Ok(Self {
            rpc_client,
            processed_blocks: DashMap::new(),
            current_slot,
            polling_interval,
            max_blocks,
            blocks_processed: 0,
            transactions_processed: 0,
        })
    }

    /// Start the main scanning loop
    ///
    /// This method runs indefinitely, polling for new blocks and processing their transactions
    ///
    /// # Returns
    /// * `Result<()>` - Success or error if scanning fails
    pub async fn start_scanning(&mut self) -> Result<()> {
        info!("Starting blockchain transaction scanning loop");
        
        if self.max_blocks > 0 {
            info!("Will process maximum {} blocks", self.max_blocks);
        } else {
            info!("Will process blocks indefinitely");
        }

        // Create interval timer for polling
        let mut poll_timer = interval(self.polling_interval);

        loop {
            // Wait for next polling interval
            poll_timer.tick().await;

            // Check if we've reached the maximum block limit
            if self.max_blocks > 0 && self.blocks_processed >= self.max_blocks {
                info!("Reached maximum block limit ({}), stopping scanner", self.max_blocks);
                break;
            }

            // Process the next block
            match self.process_next_block().await {
                Ok(processed) => {
                    if processed {
                        self.blocks_processed += 1;
                        debug!("Total blocks processed: {}", self.blocks_processed);
                        debug!("Total transactions processed: {}", self.transactions_processed);
                    }
                }
                Err(e) => {
                    error!("Error processing block at slot {}: {}", self.current_slot, e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        info!("Scanning completed. Blocks processed: {}, Transactions processed: {}", 
              self.blocks_processed, self.transactions_processed);
        Ok(())
    }

    /// Process the next block in sequence
    ///
    /// # Returns
    /// * `Result<bool>` - True if block was processed, False if block not yet available
    async fn process_next_block(&mut self) -> Result<bool> {
        // Check if block has already been processed
        if self.processed_blocks.contains_key(&self.current_slot) {
            debug!("⏭️  Block at slot {} already processed, skipping", self.current_slot);
            self.current_slot += 1;
            return Ok(false);
        }

        // Attempt to get block data from RPC (use spawn_blocking for sync RPC call)
        debug!("🔍 Fetching block data for slot: {}", self.current_slot);
        
        let block_result = tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            let slot = self.current_slot;
            move || {
                client.get_block_with_config(
                    slot,
                    solana_client::rpc_config::RpcBlockConfig {
                        encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                        transaction_details: Some(
                            solana_transaction_status::TransactionDetails::Full,
                        ),
                        rewards: Some(false),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                    },
                )
            }
        })
        .await
        .context("Task join error")?;
        
        match block_result {
            Ok(block) => {
                // Successfully retrieved block, process it
                let processed_block = self.create_processed_block(&block, self.current_slot)?;
                
                // Process all transactions in the block
                self.process_block_transactions(&block, &processed_block).await?;
                
                // Cache the processed block
                self.processed_blocks.insert(self.current_slot, processed_block);
                
                // Move to next slot
                self.current_slot += 1;
                Ok(true)
            }
            Err(e) => {
                // Block might not be available yet or other error occurred
                debug!("⏳ Block at slot {} not available yet: {}", self.current_slot, e);
                
                // Check if we need to skip ahead (block might be missing)
                let latest_slot = tokio::task::spawn_blocking({
                    let client = Arc::clone(&self.rpc_client);
                    move || client.get_slot()
                })
                .await
                .context("Task join error")?
                .context("Failed to get latest slot")?;
                
                if self.current_slot < latest_slot.saturating_sub(100) {
                    warn!("⚠️  Slot {} appears to be missing, skipping to next", self.current_slot);
                    self.current_slot += 1;
                }
                
                Ok(false)
            }
        }
    }

    /// Process all transactions in a block
    ///
    /// # Arguments
    /// * `block` - The block data from RPC
    /// * `processed_block` - Metadata about the processed block
    ///
    /// # Returns
    /// * `Result<()>` - Success or error if transaction processing fails
    async fn process_block_transactions(
        &mut self,
        block: &solana_transaction_status::UiConfirmedBlock,
        processed_block: &ProcessedBlock,
    ) -> Result<()> {
        // Get transactions from the block
        let transactions = match &block.transactions {
            Some(txs) => txs,
            None => {
                debug!("No transactions in block {}", processed_block.slot);
                return Ok(());
            }
        };

        // Process each transaction and filter for Pump.fun buy transactions
        for (tx_index, transaction) in transactions.iter().enumerate() {
            // Check if this transaction involves Pump.fun program
            if self.is_pump_fun_transaction(transaction) {
                // Only process if it's a buy transaction
                if self.has_instruction_type(transaction, &BUY_INSTRUCTION_DISCRIMINATOR) {
                    // Check if the transaction was successful
                    let is_successful = transaction.meta.as_ref()
                        .map(|meta| meta.err.is_none())
                        .unwrap_or(false);
                    
                    // Skip failed transactions
                    if !is_successful {
                        debug!("Skipping failed buy transaction");
                        continue;
                    }
                    
                    // Check if the token is new (less than an hour old)
                    match self.is_token_new(transaction).await {
                        Ok(is_new) => {
                            if is_new {
                                // Only process and log if the token is new
                                match self.process_single_transaction(transaction, processed_block, tx_index).await {
                                    Ok(processed_tx) => {
                                        info!("Found new token (<1 hour old) BUY transaction: {}", processed_tx.signature);
                                        
                                        // Extract mint address for clarity
                                        if let Some(mint_address) = self.extract_mint_address_from_buy(transaction) {
                                            info!("  Token Mint: {}", mint_address);
                                        }
                                        
                                        info!("  Slot: {} Block: {}", processed_tx.slot, processed_tx.block_hash);
                                        self.transactions_processed += 1;
                                    }
                                    Err(e) => {
                                        debug!("Failed to process Pump.fun transaction in block {}: {}", 
                                              processed_block.slot, e);
                                    }
                                }
                            } else {
                                debug!("Skipping BUY transaction for token older than 1 hour");
                            }
                        }
                        Err(e) => {
                            debug!("Error checking token age: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a transaction involves the Pump.fun program
    ///
    /// # Arguments
    /// * `transaction` - The transaction to check
    ///
    /// # Returns
    /// * `bool` - True if the transaction involves Pump.fun program
    fn is_pump_fun_transaction(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
    ) -> bool {
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // Check if Pump.fun program ID is in account keys
                        parsed_message.account_keys.iter()
                            .any(|key| key.pubkey == PUMP_FUN_PROGRAM_ID)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Check if Pump.fun program ID is in account keys
                        raw_message.account_keys.iter()
                            .any(|key| key == PUMP_FUN_PROGRAM_ID)
                    }
                }
            }
            _ => {
                // For non-JSON formats, we can't easily check, so return false
                false
            }
        }
    }

    /// Check if a transaction contains a specific Pump.fun instruction type
    ///
    /// # Arguments
    /// * `transaction` - The transaction to check
    /// * `discriminator` - The 8-byte instruction discriminator to look for
    ///
    /// # Returns
    /// * `bool` - True if the transaction contains the specified instruction type
    fn has_instruction_type(
        &self, 
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        discriminator: &[u8; 8],
    ) -> bool {
        // Process instructions based on transaction type
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, we need to check each instruction 
                        // differently since they have a different structure
                        for instruction in &parsed_message.instructions {
                            // For parsed instructions, we need to get program_id and data
                            // differently based on whether it's a parsed instruction or not
                            let (program_id, data) = match instruction {
                                solana_transaction_status::UiInstruction::Parsed(parsed_instruction) => {
                                    // For parsed instructions, we don't have direct access to raw data
                                    // Let's skip them for now and rely on the compiled instructions
                                    continue;
                                },
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Get program ID from account keys and program_id_index
                                    let program_idx = compiled.program_id_index as usize;
                                    if program_idx >= parsed_message.account_keys.len() {
                                        continue;
                                    }

                                    let program_id = &parsed_message.account_keys[program_idx].pubkey;
                                    if program_id != PUMP_FUN_PROGRAM_ID {
                                        continue;
                                    }

                                    (program_id, &compiled.data)
                                }
                            };

                            // Now check the instruction data for the discriminator
                            // Decode instruction data from base58
                            if let Ok(decoded_data) = bs58::decode(data).into_vec() {
                                // Check if data starts with our target discriminator
                                if decoded_data.len() >= 8 && decoded_data[0..8] == discriminator[..] {
                                    return true;
                                }
                            }
                        }
                        false
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // Handle raw instructions
                        for instruction in &raw_message.instructions {
                            // For raw instructions, we need to map program_id index to the actual ID
                            let program_id = if instruction.program_id_index < raw_message.account_keys.len() as u8 {
                                &raw_message.account_keys[instruction.program_id_index as usize]
                            } else {
                                continue;
                            };
                            
                            if program_id == PUMP_FUN_PROGRAM_ID {
                                // Decode instruction data from base58
                                if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                    // Check if data starts with our target discriminator
                                    if data.len() >= 8 && data[0..8] == discriminator[..] {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                }
            }
            _ => return false,
        }
    }

    /// Determine the Pump.fun instruction type for a transaction
    ///
    /// # Arguments
    /// * `transaction` - The transaction to analyze
    ///
    /// # Returns
    /// * `Option<PumpFunInstructionType>` - The instruction type if it's a Pump.fun transaction
    fn determine_pump_fun_instruction_type(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
    ) -> Option<PumpFunInstructionType> {
        if !self.is_pump_fun_transaction(transaction) {
            return None;
        }

        if self.has_instruction_type(transaction, &BUY_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Buy)
        } else if self.has_instruction_type(transaction, &SELL_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Sell)
        } else if self.has_instruction_type(transaction, &CREATE_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Create)
        } else if self.has_instruction_type(transaction, &MIGRATE_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::Migrate)
        } else if self.has_instruction_type(transaction, &SET_CREATOR_INSTRUCTION_DISCRIMINATOR) {
            Some(PumpFunInstructionType::SetCreator)
        } else {
            Some(PumpFunInstructionType::Other)
        }
    }

    /// Process a single transaction and extract relevant information
    ///
    /// # Arguments
    /// * `transaction` - The transaction data from the block
    /// * `processed_block` - Metadata about the block containing this transaction
    /// * `tx_index` - Index of the transaction within the block
    ///
    /// # Returns
    /// * `Result<ProcessedTransaction>` - Processed transaction data or error
    async fn process_single_transaction(
        &self,
        transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta,
        processed_block: &ProcessedBlock,
        tx_index: usize,
    ) -> Result<ProcessedTransaction> {
        // Extract transaction signature (first signature is the transaction signature)
        let signature = match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                ui_transaction.signatures.first()
                    .ok_or_else(|| anyhow::anyhow!("Transaction missing signature"))?
                    .clone()
            }
            solana_transaction_status::EncodedTransaction::LegacyBinary(_) => {
                return Err(anyhow::anyhow!("Legacy binary format not supported"));
            }
            solana_transaction_status::EncodedTransaction::Binary(_, _) => {
                return Err(anyhow::anyhow!("Binary format not supported"));
            }
            solana_transaction_status::EncodedTransaction::Accounts(_) => {
                return Err(anyhow::anyhow!("Accounts format not supported"));
            }
        };

        // Determine if transaction was successful
        let is_successful = transaction.meta.as_ref()
            .map(|meta| meta.err.is_none())
            .unwrap_or(false);

        // Extract error message if transaction failed
        let error_message = transaction.meta.as_ref()
            .and_then(|meta| meta.err.as_ref())
            .map(|err| format!("{:?}", err));

        // Extract fee information
        let fee = transaction.meta.as_ref()
            .map(|meta| meta.fee)
            .unwrap_or(0);

        // Extract compute units consumed (handle OptionSerializer)
        let compute_units_consumed = transaction.meta.as_ref()
            .and_then(|meta| {
                match &meta.compute_units_consumed {
                    solana_transaction_status::option_serializer::OptionSerializer::Some(units) => Some(*units),
                    solana_transaction_status::option_serializer::OptionSerializer::None => None,
                    solana_transaction_status::option_serializer::OptionSerializer::Skip => None,
                }
            });

        // Extract account keys and recent blockhash from transaction message
        let (account_keys, recent_blockhash, instruction_count) = match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        let account_keys = parsed_message.account_keys.iter()
                            .map(|key| key.pubkey.clone())
                            .collect();
                        let recent_blockhash = parsed_message.recent_blockhash.clone();
                        let instruction_count = parsed_message.instructions.len();
                        (account_keys, recent_blockhash, instruction_count)
                    }
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        let account_keys = raw_message.account_keys.clone();
                        let recent_blockhash = raw_message.recent_blockhash.clone();
                        let instruction_count = raw_message.instructions.len();
                        (account_keys, recent_blockhash, instruction_count)
                    }
                }
            }
            _ => {
                // For non-JSON formats, provide defaults
                (vec![], format!("unknown_{}", tx_index), 0)
            }
        };

        // Check if this is a Pump.fun transaction and determine transaction type
        let is_pump_fun_transaction = account_keys.iter().any(|key| key == PUMP_FUN_PROGRAM_ID);
        let pump_fun_instruction_type = if is_pump_fun_transaction {
            self.determine_pump_fun_instruction_type(transaction)
        } else {
            None
        };

        Ok(ProcessedTransaction {
            signature,
            slot: processed_block.slot,
            block_hash: processed_block.blockhash.clone(),
            is_successful,
            error_message,
            instruction_count,
            compute_units_consumed,
            fee,
            account_keys,
            recent_blockhash,
            is_pump_fun_transaction,
            pump_fun_instruction_type,
            processed_at: Utc::now(),
        })
    }

    /// Create a ProcessedBlock from RPC block data
    ///
    /// # Arguments
    /// * `block` - Block data from RPC
    /// * `slot` - Slot number
    ///
    /// # Returns
    /// * `Result<ProcessedBlock>` - Processed block metadata
    fn create_processed_block(
        &self,
        block: &solana_transaction_status::UiConfirmedBlock,
        slot: u64,
    ) -> Result<ProcessedBlock> {
        // Extract block timestamp if available
        let timestamp = block.block_time.map(|ts| {
            DateTime::from_timestamp(ts, 0)
                .unwrap_or_else(|| Utc::now())
        });

        // Get transaction count
        let transaction_count = block.transactions.as_ref()
            .map(|txs| txs.len())
            .unwrap_or(0);

        // Create processed block record
        let processed_block = ProcessedBlock {
            slot,
            blockhash: block.blockhash.clone(),
            parent_slot: block.parent_slot,
            timestamp,
            transaction_count,
            processed_at: Utc::now(),
        };

        debug!("✅ Created processed block record for slot {}", slot);
        Ok(processed_block)
    }

    /// Get statistics about the scanning process
    ///
    /// # Returns
    /// * Statistics about processed blocks and transactions
    pub fn get_stats(&self) -> (u64, u64, usize) {
        (self.blocks_processed, self.transactions_processed, self.processed_blocks.len())
    }

    /// Check if a token is less than an hour old
    ///
    /// # Arguments
    /// * `transaction` - The transaction containing the buy instruction
    ///
    /// # Returns
    /// * `Result<bool>` - Whether the token is less than an hour old
    async fn is_token_new(&self, transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> Result<bool> {
        // Extract the mint account address from the buy instruction
        let mint_address = match self.extract_mint_address_from_buy(transaction) {
            Some(address) => address,
            None => {
                debug!("Could not extract mint address from buy transaction");
                return Ok(false);
            }
        };
        
        debug!("Found mint address: {}", mint_address);
        
        // Fetch the mint account data using RPC
        let mint_account = match tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            let mint_pubkey = mint_address.parse().context("Failed to parse mint address")?;
            move || client.get_account(&mint_pubkey)
        })
        .await
        .context("Task join error")?
        {
            Ok(account) => account,
            Err(e) => {
                debug!("Failed to fetch mint account {}: {}", mint_address, e);
                return Ok(false);
            }
        };
        
        // Get current slot and timestamp to compare with
        let current_slot = tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            move || client.get_slot()
        })
        .await
        .context("Task join error")?
        .context("Failed to get current slot")?;
        
        // Get current blockchain time
        let current_time = tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            move || client.get_block_time(current_slot)
        })
        .await
        .context("Task join error")?
        .context("Failed to get block time")?;
        
        // Token age approach: Check account metadata and transaction history
        
        // 1. Check SPL token mint data to determine if it's a valid token mint account
        // For SPL tokens, we could check the mint authority, supply, etc.
        if mint_account.executable {
            // Executable accounts can't be SPL token mints
            return Ok(false);
        }
        
        // SPL tokens should be owned by the Token Program
        let spl_token_program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        let spl_token_2022_program_id = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
        
        if mint_account.owner.to_string() != spl_token_program_id && 
           mint_account.owner.to_string() != spl_token_2022_program_id {
            debug!("Account is not owned by token program");
            return Ok(false);
        }
        
        // 2. Get transaction history for this mint account to determine age
        let signatures = tokio::task::spawn_blocking({
            let client = Arc::clone(&self.rpc_client);
            let mint_pubkey = mint_address.parse().context("Failed to parse mint address")?;
            move || client.get_signatures_for_address(&mint_pubkey)
        })
        .await
        .context("Task join error")?;
        
        match signatures {
            Ok(signatures) => {
                // If there are very few signatures (transactions), it's likely new
                if !signatures.is_empty() {
                    // Check timestamp of oldest signature, which should be the creation tx
                    if let Some(oldest_tx) = signatures.last() {
                        if let Some(block_time) = oldest_tx.block_time {
                            // Calculate time difference in seconds
                            let time_diff = current_time - block_time;
                            
                            // If less than an hour old (3600 seconds)
                            if time_diff < 3600 {
                                info!("Token is new: created {} seconds ago", time_diff);
                                return Ok(true);
                            } else {
                                debug!("Token is {} seconds old (>1 hour)", time_diff);
                                return Ok(false);
                            }
                        }
                    }
                }
                
                // If we reach here, we couldn't determine the age from transaction history
                // As a fallback, check if the account data length is standard for SPL token
                // and if the account has few transactions - this is a heuristic for new tokens
                if signatures.len() <= 3 {
                    // Typical SPL token mint has specific data length
                    // This is a heuristic for new tokens that might have not indexed timestamps yet
                    debug!("Token has very few transactions ({}), considering it new", signatures.len());
                    return Ok(true);
                }
            }
            Err(e) => {
                debug!("Failed to get signatures for mint account: {}", e);
                // If we can't get signatures, we might be rate limited or the account is very new
                // In that case, assume it's new if we're looking at a buy transaction
                return Ok(true);
            }
        }
        
        debug!("Token does not appear to be new (>1 hour old)");
        Ok(false)
    }
    
    /// Extract the mint address from a buy transaction
    ///
    /// # Arguments
    /// * `transaction` - The transaction containing the buy instruction
    ///
    /// # Returns
    /// * `Option<String>` - The mint address if found
    fn extract_mint_address_from_buy(&self, transaction: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> Option<String> {
        // Find the buy instruction
        match &transaction.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed_message) => {
                        // For parsed messages, we need to check each instruction
                        for instruction in &parsed_message.instructions {
                            match instruction {
                                solana_transaction_status::UiInstruction::Compiled(compiled) => {
                                    // Check if this is the buy instruction
                                    if let Ok(data) = bs58::decode(&compiled.data).into_vec() {
                                        if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                            // According to the Pump.fun IDL, the mint account is the 3rd account (index 2)
                                            // in the buy instruction
                                            if compiled.accounts.len() > 2 {
                                                let mint_idx = compiled.accounts[2] as usize;
                                                if mint_idx < parsed_message.account_keys.len() {
                                                    return Some(parsed_message.account_keys[mint_idx].pubkey.clone());
                                                }
                                            }
                                        }
                                    }
                                },
                                _ => continue,
                            }
                        }
                    },
                    solana_transaction_status::UiMessage::Raw(raw_message) => {
                        // For raw messages, check each instruction
                        for instruction in &raw_message.instructions {
                            // Check if this is the buy instruction
                            if let Ok(data) = bs58::decode(&instruction.data).into_vec() {
                                if data.len() >= 8 && data[0..8] == BUY_INSTRUCTION_DISCRIMINATOR {
                                    // The mint account is the 3rd account (index 2) in the accounts list
                                    if instruction.accounts.len() > 2 {
                                        let mint_idx = instruction.accounts[2] as usize;
                                        if mint_idx < raw_message.account_keys.len() {
                                            return Some(raw_message.account_keys[mint_idx].clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            _ => return None,
        }
        
        None
    }

    /// Log comprehensive Pump.fun transaction information to console
    ///
    /// # Arguments
    /// * `tx` - The processed transaction to log
    fn log_pump_fun_transaction(&self, tx: &ProcessedTransaction) {
        // Only log buy transactions
        if tx.pump_fun_instruction_type != Some(PumpFunInstructionType::Buy) {
            return;
        }
        
        // Check if the token is less than an hour old
        // Note: We need to use tokio::spawn_blocking or spawn a task here
        // since we're in a synchronous function but need to make async RPC calls
        
        // For now, we'll just add a placeholder for future implementation
        // and log basic buy transaction information
        
        info!("Found BUY transaction: {}", tx.signature);
        info!("  Checking if token is new (<1 hour)...");
        debug!("  Will be checking mint account age in process_block_transactions");
    }
}

/// Main application entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if present
    dotenv::dotenv().ok();

    // Initialize logging system
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();

    // Parse command line arguments
    let args = Args::parse();

    info!("Starting Solana Pump.fun Transaction Scanner v{}", env!("CARGO_PKG_VERSION"));
    info!("This scanner specifically monitors Pump.fun transactions");
    info!("Configuration:");
    info!("  RPC URL: {}", args.rpc_url);
    info!("  Polling Interval: {}s", args.interval);
    info!("  Start Slot: {}", if args.start_slot == 0 { "latest".to_string() } else { args.start_slot.to_string() });
    info!("  Max Blocks: {}", if args.max_blocks == 0 { "unlimited".to_string() } else { args.max_blocks.to_string() });

    // Create and start the scanner
    let mut scanner = SolanaBlockScanner::new(
        args.rpc_url,
        args.start_slot,
        Duration::from_secs(args.interval),
        args.max_blocks,
    )
    .await
    .context("Failed to initialize transaction scanner")?;

    // Handle graceful shutdown on Ctrl+C
    tokio::select! {
        result = scanner.start_scanning() => {
            match result {
                Ok(()) => info!("Scanner completed successfully"),
                Err(e) => error!("Scanner failed: {}", e),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received interrupt signal, shutting down gracefully...");
            let (blocks_processed, transactions_processed, cached_blocks) = scanner.get_stats();
            info!("Final Statistics:");
            info!("  Blocks Processed: {}", blocks_processed);
            info!("  Pump.fun Transactions Found: {}", transactions_processed);
            info!("  Cached Blocks: {}", cached_blocks);
        }
    }

    info!("Solana Pump.fun Transaction Scanner shutdown complete");
    Ok(())
} 