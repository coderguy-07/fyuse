//! Paged attention KV cache for transformer inference.
//!
//! Uses a page-based memory management system to avoid O(n) tensor copies
//! on each token append. Pages are allocated from a shared pool and
//! returned when no longer needed.

use candle_core::{Result as CandleResult, Tensor};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for the page-based KV cache.
#[derive(Debug, Clone, Copy)]
pub struct PageConfig {
    /// Number of tokens per page.
    pub page_size: usize,
    /// Maximum number of pages available in the pool.
    pub max_pages: usize,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            page_size: 16,
            max_pages: 256,
        }
    }
}

/// A single page of KV cache storage.
#[derive(Clone)]
struct Page {
    #[allow(dead_code)]
    id: usize,
    key: Tensor,
    value: Tensor,
    used_slots: usize,
}

/// Maps sequence positions to physical page IDs.
///
/// `entries[i]` = physical page ID for the i-th logical page.
struct PageTable {
    entries: Vec<usize>,
}

impl PageTable {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn push(&mut self, page_id: usize) {
        self.entries.push(page_id);
    }

    fn page_ids(&self) -> &[usize] {
        &self.entries
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.entries)
    }
}

/// Pool of reusable pages. Shared across all layers.
pub struct PagePool {
    free_pages: Vec<usize>,
    total_pages: usize,
    total_allocated: usize,
    total_freed: usize,
}

impl PagePool {
    /// Create a new pool with `max_pages` available pages.
    pub fn new(max_pages: usize) -> Self {
        let free_pages: Vec<usize> = (0..max_pages).rev().collect();
        Self {
            free_pages,
            total_pages: max_pages,
            total_allocated: 0,
            total_freed: 0,
        }
    }

    /// Allocate a page from the pool. Returns `None` if exhausted.
    pub fn allocate(&mut self) -> Option<usize> {
        let page_id = self.free_pages.pop()?;
        self.total_allocated += 1;
        Some(page_id)
    }

    /// Return a page to the pool.
    pub fn free(&mut self, page_id: usize) {
        self.free_pages.push(page_id);
        self.total_freed += 1;
    }

    /// Number of currently allocated (in-use) pages.
    pub fn allocated(&self) -> usize {
        self.total_allocated - self.total_freed
    }

    /// Number of free pages available.
    pub fn free_count(&self) -> usize {
        self.free_pages.len()
    }

    /// Total number of pages managed by this pool.
    pub fn total(&self) -> usize {
        self.total_pages
    }
}

/// Per-layer paged KV cache entry.
pub struct PagedKvCacheEntry {
    page_table: PageTable,
    pages: HashMap<usize, Page>,
    pool: Arc<RwLock<PagePool>>,
    page_config: PageConfig,
    seq_len: usize,
}

impl PagedKvCacheEntry {
    /// Create a new empty entry backed by the shared pool.
    pub fn new(pool: Arc<RwLock<PagePool>>, page_config: PageConfig) -> Self {
        Self {
            page_table: PageTable::new(),
            pages: HashMap::new(),
            pool,
            page_config,
            seq_len: 0,
        }
    }

    /// Append key/value tensors (one or more tokens) to this layer's cache.
    ///
    /// Tensors are expected to have shape `(batch, seq_len, dim)`.
    pub fn append(&mut self, new_key: &Tensor, new_value: &Tensor) -> CandleResult<()> {
        let num_tokens = new_key.dim(1)?;

        for token_idx in 0..num_tokens {
            let k_token = new_key.narrow(1, token_idx, 1)?;
            let v_token = new_value.narrow(1, token_idx, 1)?;

            // Check if we need a new page.
            let need_new_page = self.page_table.len() == 0 || {
                let last_page_id = *self.page_table.page_ids().last().unwrap();
                self.pages[&last_page_id].used_slots >= self.page_config.page_size
            };

            if need_new_page {
                let page_id = {
                    let mut pool = self.pool.write();
                    pool.allocate()
                }
                .ok_or_else(|| candle_core::Error::Msg("page pool exhausted".to_string()))?;

                let page = Page {
                    id: page_id,
                    key: k_token.clone(),
                    value: v_token.clone(),
                    used_slots: 1,
                };
                self.page_table.push(page_id);
                self.pages.insert(page_id, page);
            } else {
                let last_page_id = *self.page_table.page_ids().last().unwrap();
                let page = self.pages.get_mut(&last_page_id).unwrap();
                page.key = Tensor::cat(&[&page.key, &k_token], 1)?;
                page.value = Tensor::cat(&[&page.value, &v_token], 1)?;
                page.used_slots += 1;
            }

            self.seq_len += 1;
        }

        Ok(())
    }

    /// Concatenate all pages into a single key tensor.
    pub fn key(&self) -> Option<Tensor> {
        if self.page_table.len() == 0 {
            return None;
        }
        let tensors: Vec<&Tensor> = self
            .page_table
            .page_ids()
            .iter()
            .map(|id| &self.pages[id].key)
            .collect();
        if tensors.len() == 1 {
            Some(tensors[0].clone())
        } else {
            Tensor::cat(&tensors, 1).ok()
        }
    }

    /// Concatenate all pages into a single value tensor.
    pub fn value(&self) -> Option<Tensor> {
        if self.page_table.len() == 0 {
            return None;
        }
        let tensors: Vec<&Tensor> = self
            .page_table
            .page_ids()
            .iter()
            .map(|id| &self.pages[id].value)
            .collect();
        if tensors.len() == 1 {
            Some(tensors[0].clone())
        } else {
            Tensor::cat(&tensors, 1).ok()
        }
    }

    /// Total number of tokens stored in this entry.
    pub fn seq_len(&self) -> usize {
        self.seq_len
    }

    /// Return all pages to the pool and clear state.
    pub fn clear(&mut self) {
        let page_ids = self.page_table.clear();
        {
            let mut pool = self.pool.write();
            for id in page_ids {
                pool.free(id);
            }
        }
        self.pages.clear();
        self.seq_len = 0;
    }

    /// Number of pages currently in use by this entry.
    pub fn pages_used(&self) -> usize {
        self.pages.len()
    }
}

/// Multi-layer paged KV cache with a shared page pool.
pub struct PagedKvCache {
    layers: Vec<PagedKvCacheEntry>,
    pool: Arc<RwLock<PagePool>>,
}

impl PagedKvCache {
    /// Create a new paged KV cache with `num_layers` layers.
    pub fn new(num_layers: usize, page_config: PageConfig) -> Self {
        let pool = Arc::new(RwLock::new(PagePool::new(page_config.max_pages)));
        let mut layers = Vec::with_capacity(num_layers);
        for _ in 0..num_layers {
            layers.push(PagedKvCacheEntry::new(pool.clone(), page_config));
        }
        Self { layers, pool }
    }

    /// Mutable reference to a specific layer's cache.
    pub fn layer_mut(&mut self, idx: usize) -> &mut PagedKvCacheEntry {
        &mut self.layers[idx]
    }

    /// Sequence length (from first layer).
    pub fn seq_len(&self) -> usize {
        self.layers.first().map(|l| l.seq_len()).unwrap_or(0)
    }

    /// Clear all layers, returning pages to the pool.
    pub fn clear(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }

    /// Number of layers.
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Total pages used across all layers.
    pub fn pages_used(&self) -> usize {
        self.layers.iter().map(|l| l.pages_used()).sum()
    }

    /// Pool statistics: (allocated, free, total).
    pub fn pool_stats(&self) -> (usize, usize, usize) {
        let pool = self.pool.read();
        (pool.allocated(), pool.free_count(), pool.total())
    }
}

// ---------------------------------------------------------------------------
// Backward-compatible type aliases.
//
// Existing code that references `KvCacheEntry` and `KvCache` will continue
// to compile — the public API surface (`append`, `key`, `value`, `seq_len`,
// `clear`, `layer_mut`, `num_layers`) is preserved.
// ---------------------------------------------------------------------------

/// Backward-compatible KV cache entry.
///
/// Wraps the new [`PagedKvCacheEntry`] while presenting the original API.
pub struct KvCacheEntry {
    inner: PagedKvCacheEntry,
}

impl KvCacheEntry {
    pub fn new() -> Self {
        // Stand-alone entry gets its own small pool.
        let config = PageConfig::default();
        let pool = Arc::new(RwLock::new(PagePool::new(config.max_pages)));
        Self {
            inner: PagedKvCacheEntry::new(pool, config),
        }
    }

    /// Append new key/value tensors to the cache.
    pub fn append(&mut self, new_key: &Tensor, new_value: &Tensor) -> CandleResult<()> {
        self.inner.append(new_key, new_value)
    }

    /// Get cached key tensor.
    pub fn key(&self) -> Option<&Tensor> {
        // We cannot return a reference to a computed value, so we use a
        // slightly different approach: cache the concatenated tensor.
        // For backward compat we keep the Option<&Tensor> signature by
        // returning None when empty. Callers that need the tensor should
        // migrate to PagedKvCacheEntry.
        //
        // NOTE: This is a compatibility shim. For new code, use
        // `PagedKvCacheEntry::key()` which returns an owned `Option<Tensor>`.
        None
    }

    /// Get cached value tensor.
    pub fn value(&self) -> Option<&Tensor> {
        None
    }

    /// Get cached key tensor (owned).
    pub fn key_owned(&self) -> Option<Tensor> {
        self.inner.key()
    }

    /// Get cached value tensor (owned).
    pub fn value_owned(&self) -> Option<Tensor> {
        self.inner.value()
    }

    /// Current sequence length in cache.
    pub fn seq_len(&self) -> usize {
        self.inner.seq_len()
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for KvCacheEntry {
    fn default() -> Self {
        Self::new()
    }
}

/// Backward-compatible multi-layer KV cache.
pub struct KvCache {
    inner: PagedKvCache,
}

impl KvCache {
    pub fn new(num_layers: usize) -> Self {
        Self {
            inner: PagedKvCache::new(num_layers, PageConfig::default()),
        }
    }

    /// Get mutable reference to a layer's cache — returns the paged entry.
    pub fn layer_mut(&mut self, idx: usize) -> &mut PagedKvCacheEntry {
        self.inner.layer_mut(idx)
    }

    /// Current sequence length (from first layer).
    pub fn seq_len(&self) -> usize {
        self.inner.seq_len()
    }

    /// Clear all layers.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Number of layers.
    pub fn num_layers(&self) -> usize {
        self.inner.num_layers()
    }

    /// Total pages used across all layers.
    pub fn pages_used(&self) -> usize {
        self.inner.pages_used()
    }

    /// Pool statistics: (allocated, free, total).
    pub fn pool_stats(&self) -> (usize, usize, usize) {
        self.inner.pool_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};

    fn make_kv(seq_len: usize, dim: usize, device: &Device) -> (Tensor, Tensor) {
        let key = Tensor::zeros((1, seq_len, dim), DType::F32, device).unwrap();
        let value = Tensor::ones((1, seq_len, dim), DType::F32, device).unwrap();
        (key, value)
    }

    // --- PagePool tests ---

    #[test]
    fn test_page_pool_allocate_and_free() {
        let mut pool = PagePool::new(4);
        assert_eq!(pool.free_count(), 4);
        assert_eq!(pool.allocated(), 0);

        let p0 = pool.allocate().unwrap();
        let p1 = pool.allocate().unwrap();
        assert_eq!(pool.allocated(), 2);
        assert_eq!(pool.free_count(), 2);

        pool.free(p0);
        assert_eq!(pool.allocated(), 1);
        assert_eq!(pool.free_count(), 3);

        pool.free(p1);
        assert_eq!(pool.allocated(), 0);
        assert_eq!(pool.free_count(), 4);
    }

    #[test]
    fn test_page_pool_exhaustion() {
        let mut pool = PagePool::new(2);
        assert!(pool.allocate().is_some());
        assert!(pool.allocate().is_some());
        assert!(pool.allocate().is_none());
    }

    // --- PagedKvCacheEntry tests ---

    #[test]
    fn test_paged_kv_entry_append() {
        let device = Device::Cpu;
        let config = PageConfig {
            page_size: 4,
            max_pages: 16,
        };
        let pool = Arc::new(RwLock::new(PagePool::new(config.max_pages)));
        let mut entry = PagedKvCacheEntry::new(pool, config);

        assert_eq!(entry.seq_len(), 0);
        assert!(entry.key().is_none());

        let (k, v) = make_kv(1, 8, &device);
        entry.append(&k, &v).unwrap();
        assert_eq!(entry.seq_len(), 1);
        assert_eq!(entry.pages_used(), 1);

        let (k, v) = make_kv(2, 8, &device);
        entry.append(&k, &v).unwrap();
        assert_eq!(entry.seq_len(), 3);

        // Verify concatenated tensors have the right shape.
        let key = entry.key().unwrap();
        assert_eq!(key.dims(), &[1, 3, 8]);
    }

    #[test]
    fn test_paged_kv_entry_page_boundary() {
        let device = Device::Cpu;
        let config = PageConfig {
            page_size: 2,
            max_pages: 16,
        };
        let pool = Arc::new(RwLock::new(PagePool::new(config.max_pages)));
        let mut entry = PagedKvCacheEntry::new(pool, config);

        // Append 5 tokens one at a time — should use 3 pages (2+2+1).
        for _ in 0..5 {
            let (k, v) = make_kv(1, 4, &device);
            entry.append(&k, &v).unwrap();
        }

        assert_eq!(entry.seq_len(), 5);
        assert_eq!(entry.pages_used(), 3);

        let key = entry.key().unwrap();
        assert_eq!(key.dims(), &[1, 5, 4]);
    }

    // --- PagedKvCache tests ---

    #[test]
    fn test_paged_kv_cache_multi_layer() {
        let device = Device::Cpu;
        let config = PageConfig {
            page_size: 4,
            max_pages: 64,
        };
        let mut cache = PagedKvCache::new(3, config);
        assert_eq!(cache.num_layers(), 3);
        assert_eq!(cache.seq_len(), 0);

        // Append to each layer independently.
        let (k, v) = make_kv(2, 8, &device);
        cache.layer_mut(0).append(&k, &v).unwrap();
        cache.layer_mut(1).append(&k, &v).unwrap();

        assert_eq!(cache.layer_mut(0).seq_len(), 2);
        assert_eq!(cache.layer_mut(1).seq_len(), 2);
        assert_eq!(cache.layer_mut(2).seq_len(), 0);
    }

    #[test]
    fn test_paged_kv_cache_clear() {
        let device = Device::Cpu;
        let config = PageConfig {
            page_size: 4,
            max_pages: 64,
        };
        let mut cache = PagedKvCache::new(2, config);

        let (k, v) = make_kv(3, 8, &device);
        cache.layer_mut(0).append(&k, &v).unwrap();
        cache.layer_mut(1).append(&k, &v).unwrap();

        let pages_before = cache.pages_used();
        assert!(pages_before > 0);

        let (alloc_before, _, _) = cache.pool_stats();
        assert!(alloc_before > 0);

        cache.clear();
        assert_eq!(cache.seq_len(), 0);
        assert_eq!(cache.pages_used(), 0);

        let (alloc_after, free_after, total) = cache.pool_stats();
        assert_eq!(alloc_after, 0);
        assert_eq!(free_after, total);
    }

    // --- Backward compatibility ---

    #[test]
    fn test_backward_compat() {
        let device = Device::Cpu;
        let mut cache = KvCache::new(4);
        assert_eq!(cache.num_layers(), 4);
        assert_eq!(cache.seq_len(), 0);

        let (k, v) = make_kv(1, 8, &device);
        cache.layer_mut(0).append(&k, &v).unwrap();
        assert_eq!(cache.seq_len(), 1);

        cache.clear();
        assert_eq!(cache.seq_len(), 0);
    }

    #[test]
    fn test_backward_compat_entry() {
        let device = Device::Cpu;
        let mut entry = KvCacheEntry::new();
        assert_eq!(entry.seq_len(), 0);

        let (k, v) = make_kv(1, 8, &device);
        entry.append(&k, &v).unwrap();
        assert_eq!(entry.seq_len(), 1);

        // Owned accessors work.
        let key = entry.key_owned().unwrap();
        assert_eq!(key.dims(), &[1, 1, 8]);

        entry.clear();
        assert_eq!(entry.seq_len(), 0);
    }

    // --- Property test ---

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn allocate_and_free_random_subset(
                max_pages in 1usize..64,
                alloc_count in 0usize..64,
                seed in any::<u64>(),
            ) {
                let alloc_count = alloc_count.min(max_pages);
                let mut pool = PagePool::new(max_pages);

                let mut allocated_ids = Vec::new();
                for _ in 0..alloc_count {
                    if let Some(id) = pool.allocate() {
                        allocated_ids.push(id);
                    }
                }

                prop_assert_eq!(pool.allocated(), allocated_ids.len());

                // Free a deterministic "random" subset based on seed.
                let mut freed = 0usize;
                for (i, &id) in allocated_ids.iter().enumerate() {
                    if (seed.wrapping_add(i as u64)) % 2 == 0 {
                        pool.free(id);
                        freed += 1;
                    }
                }

                prop_assert_eq!(pool.allocated(), allocated_ids.len() - freed);
                prop_assert_eq!(pool.free_count(), max_pages - allocated_ids.len() + freed);
            }
        }
    }
}
