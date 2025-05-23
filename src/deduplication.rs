use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use log::{info, debug};

/// Transaction deduplication manager to avoid processing the same transaction twice
/// when it appears in both live stream and confirmed blocks
pub struct TransactionDeduplicator {
    // LRU cache to store recently seen transaction signatures
    cache: Arc<Mutex<LruCache<String, u64>>>,
    cache_size: usize,
}

impl TransactionDeduplicator {
    /// Create a new deduplicator with specified cache size
    pub fn new(cache_size: usize) -> Self {
        let cache_size = cache_size.max(1000); // Minimum cache size
        
        info!("Initializing transaction deduplicator with cache size: {}", cache_size);
        
        Self {
            cache: Arc::new(Mutex::new(
                LruCache::new(NonZeroUsize::new(cache_size).unwrap())
            )),
            cache_size,
        }
    }
    
    /// Check if a transaction signature has been seen before (live stream)
    /// Returns true if this is the first time seeing this signature
    pub fn mark_live_transaction(&self, signature: &str, slot: u64) -> bool {
        let mut cache = self.cache.lock().unwrap();
        
        if cache.contains(signature) {
            debug!("Live transaction already seen: {}", signature);
            false
        } else {
            debug!("New live transaction: {} at slot {}", signature, slot);
            cache.put(signature.to_string(), slot);
            true
        }
    }
    
    /// Check if a transaction from a confirmed block should be processed
    /// Returns true if this transaction hasn't been seen in the live stream
    pub fn should_process_block_transaction(&self, signature: &str, _slot: u64) -> bool {
        let cache = self.cache.lock().unwrap();
        
        let should_process = !cache.contains(signature);
        
        if should_process {
            debug!("Processing block transaction: {}", signature);
        } else {
            debug!("Skipping block transaction (already seen live): {}", signature);
        }
        
        should_process
    }
    
    /// Mark a transaction as seen from a confirmed block
    /// This is called after processing a transaction from a confirmed block
    /// to ensure we don't process it again if it shows up in another block
    pub fn mark_block_transaction(&self, signature: &str, slot: u64) {
        let mut cache = self.cache.lock().unwrap();
        
        if !cache.contains(signature) {
            debug!("Marking block transaction as processed: {} at slot {}", signature, slot);
            cache.put(signature.to_string(), slot);
        }
    }
    
    /// Get cache statistics for monitoring
    pub fn get_stats(&self) -> DeduplicationStats {
        let cache = self.cache.lock().unwrap();
        
        DeduplicationStats {
            cache_size: self.cache_size,
            current_entries: cache.len(),
            capacity: cache.cap().get(),
        }
    }
    
    /// Clear the cache (useful for testing or reset scenarios)
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        info!("Transaction deduplication cache cleared");
    }
    
    /// Check if cache needs maintenance (can be called periodically)
    pub fn needs_maintenance(&self) -> bool {
        let cache = self.cache.lock().unwrap();
        let usage_ratio = cache.len() as f64 / cache.cap().get() as f64;
        
        // Consider maintenance needed if cache is 90% full
        usage_ratio > 0.9
    }
    
    /// Get signatures currently in cache (for debugging)
    pub fn get_cached_signatures(&self) -> Vec<(String, u64)> {
        let cache = self.cache.lock().unwrap();
        cache.iter().map(|(sig, slot)| (sig.clone(), *slot)).collect()
    }
}

#[derive(Debug, Clone)]
pub struct DeduplicationStats {
    pub cache_size: usize,
    pub current_entries: usize,
    pub capacity: usize,
}

impl DeduplicationStats {
    pub fn usage_percentage(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current_entries as f64 / self.capacity as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deduplication_basic() {
        let dedup = TransactionDeduplicator::new(100);
        
        // First time seeing a transaction should return true
        assert!(dedup.mark_live_transaction("sig1", 1000));
        
        // Second time should return false
        assert!(!dedup.mark_live_transaction("sig1", 1001));
        
        // Block transaction already seen live should not be processed
        assert!(!dedup.should_process_block_transaction("sig1", 1000));
        
        // New block transaction should be processed
        assert!(dedup.should_process_block_transaction("sig2", 1001));
    }
    
    #[test]
    fn test_cache_capacity() {
        let dedup = TransactionDeduplicator::new(3);
        
        // Fill cache
        dedup.mark_live_transaction("sig1", 1000);
        dedup.mark_live_transaction("sig2", 1001);
        dedup.mark_live_transaction("sig3", 1002);
        
        let stats = dedup.get_stats();
        assert_eq!(stats.current_entries, 3);
        
        // Adding another should evict the oldest
        dedup.mark_live_transaction("sig4", 1003);
        
        let stats = dedup.get_stats();
        assert_eq!(stats.current_entries, 3);
        
        // sig1 should have been evicted
        assert!(dedup.should_process_block_transaction("sig1", 1000));
        assert!(!dedup.should_process_block_transaction("sig4", 1003));
    }
} 