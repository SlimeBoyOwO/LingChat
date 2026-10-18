use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone, Copy, Debug)]
pub struct CacheLimits {
    pub max_entries: usize,
    pub max_bytes: usize,
    pub max_entry_bytes: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoryCacheConfig {
    pub query_vectors: CacheLimits,
    pub results: CacheLimits,
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            query_vectors: CacheLimits {
                max_entries: 128,
                max_bytes: 1024 * 1024,
                max_entry_bytes: 32 * 1024,
            },
            results: CacheLimits {
                max_entries: 64,
                max_bytes: 1024 * 1024,
                max_entry_bytes: 32 * 1024,
            },
        }
    }
}

impl MemoryCacheConfig {
    pub fn disabled() -> Self {
        let disabled = CacheLimits {
            max_entries: 0,
            max_bytes: 0,
            max_entry_bytes: 0,
        };
        Self {
            query_vectors: disabled,
            results: disabled,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LruStats {
    pub hits: u64,
    pub misses: u64,
    pub insertions: u64,
    pub evictions: u64,
    pub rejected: u64,
    pub invalidations: u64,
    pub entries: usize,
    pub retained_bytes: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MemoryCacheStats {
    pub query_vectors: LruStats,
    pub results: LruStats,
    pub hybrid_searches: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CacheUsage {
    pub query_vector_hit: bool,
    pub result_hit: bool,
}

struct Entry {
    key: String,
    payload: Vec<u8>,
}

impl Entry {
    fn bytes(&self) -> usize {
        size_of::<Self>() + self.key.capacity() + self.payload.capacity()
    }
}

struct BoundedLru {
    limits: CacheLimits,
    entries: VecDeque<Entry>,
    stats: LruStats,
}

impl BoundedLru {
    fn new(limits: CacheLimits) -> Self {
        Self {
            limits,
            entries: VecDeque::new(),
            stats: LruStats::default(),
        }
    }

    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            let entry = self.entries.remove(index).unwrap();
            let payload = entry.payload.clone();
            self.entries.push_back(entry);
            self.stats.hits += 1;
            Some(payload)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            let entry = self.entries.remove(index).unwrap();
            self.stats.retained_bytes -= entry.bytes();
            self.stats.invalidations += 1;
        }
    }

    fn insert(&mut self, key: String, payload: Vec<u8>) {
        let entry = Entry { key, payload };
        let bytes = entry.bytes();
        if self.limits.max_entries == 0
            || bytes > self.limits.max_entry_bytes
            || bytes > self.limits.max_bytes
        {
            self.stats.rejected += 1;
            return;
        }
        self.remove(&entry.key);
        while self.entries.len() >= self.limits.max_entries
            || self.stats.retained_bytes + bytes > self.limits.max_bytes
        {
            let removed = self.entries.pop_front().unwrap();
            self.stats.retained_bytes -= removed.bytes();
            self.stats.evictions += 1;
        }
        self.stats.retained_bytes += bytes;
        self.stats.insertions += 1;
        self.entries.push_back(entry);
    }

    fn clear(&mut self) {
        self.stats.invalidations += self.entries.len() as u64;
        self.entries = VecDeque::new();
        self.stats.retained_bytes = 0;
    }

    fn stats(&self) -> LruStats {
        let mut stats = self.stats.clone();
        stats.entries = self.entries.len();
        stats
    }
}

pub(crate) struct MemoryCaches {
    vectors: Mutex<BoundedLru>,
    results: Mutex<BoundedLru>,
    hybrid_searches: AtomicU64,
}

impl MemoryCaches {
    pub fn new(config: MemoryCacheConfig) -> Self {
        Self {
            vectors: Mutex::new(BoundedLru::new(config.query_vectors)),
            results: Mutex::new(BoundedLru::new(config.results)),
            hybrid_searches: AtomicU64::new(0),
        }
    }

    fn cache(&self, vector: bool) -> &Mutex<BoundedLru> {
        if vector { &self.vectors } else { &self.results }
    }

    pub fn get<Value: DeserializeOwned>(&self, vector: bool, key: &str) -> Option<Value> {
        let payload = self
            .cache(vector)
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(key)?;
        match serde_json::from_slice(&payload) {
            Ok(value) => Some(value),
            Err(_) => {
                self.remove(vector, key);
                None
            },
        }
    }

    pub fn insert(&self, vector: bool, key: String, value: &impl Serialize) {
        if let Ok(payload) = serde_json::to_vec(value) {
            self.cache(vector)
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(key, payload);
        }
    }

    pub fn remove(&self, vector: bool, key: &str) {
        self.cache(vector)
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(key);
    }

    pub fn record_search(&self) {
        self.hybrid_searches.fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> MemoryCacheStats {
        MemoryCacheStats {
            query_vectors: self
                .vectors
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .stats(),
            results: self
                .results
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .stats(),
            hybrid_searches: self.hybrid_searches.load(Ordering::Relaxed),
        }
    }

    pub fn clear(&self) {
        self.vectors
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        self.results
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

#[cfg(test)]
#[path = "memory_cache_tests.rs"]
mod tests;
