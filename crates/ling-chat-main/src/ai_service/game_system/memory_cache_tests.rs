use super::*;

fn limits(entries: usize, bytes: usize) -> CacheLimits {
    CacheLimits {
        max_entries: entries,
        max_bytes: bytes,
        max_entry_bytes: bytes,
    }
}

#[test]
fn hits_and_reinsertions_refresh_lru_order() {
    let mut cache = BoundedLru::new(limits(2, 1024));
    cache.insert("first".into(), vec![1]);
    cache.insert("second".into(), vec![2]);
    assert_eq!(cache.get("first"), Some(vec![1]));
    cache.insert("third".into(), vec![3]);
    assert_eq!(cache.get("second"), None);
    cache.insert("first".into(), vec![9]);
    cache.insert("fourth".into(), vec![4]);
    assert_eq!(cache.get("third"), None);
    assert_eq!(cache.get("first"), Some(vec![9]));
    assert_eq!(cache.stats().entries, 2);
    assert_eq!(cache.stats().evictions, 2);
}

#[test]
fn byte_budget_and_single_item_limit_bound_retained_buffers() {
    let mut cache = BoundedLru::new(limits(10, 250));
    cache.insert("first".into(), vec![0; 100]);
    cache.insert("second".into(), vec![0; 100]);
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.stats().evictions, 1);
    assert!(cache.stats().retained_bytes <= 250);
    cache.insert("oversized".into(), vec![0; 251]);
    assert_eq!(cache.stats().rejected, 1);
    assert_eq!(cache.stats().entries, 1);
    cache.clear();
    assert_eq!(cache.stats().entries, 0);
    assert_eq!(cache.stats().retained_bytes, 0);
    assert_eq!(cache.stats().invalidations, 1);
}

#[test]
fn allocated_capacity_counts_even_if_payload_length_is_small() {
    let mut cache = BoundedLru::new(limits(5, 256));
    let mut payload = Vec::with_capacity(1024);
    payload.push(1);
    cache.insert("large-allocation".into(), payload);
    assert_eq!(cache.stats().entries, 0);
    assert_eq!(cache.stats().rejected, 1);
}

#[test]
fn per_item_limit_rejects_large_entries_without_evicting_valid_ones() {
    let mut cache = BoundedLru::new(CacheLimits {
        max_entries: 10,
        max_bytes: 4096,
        max_entry_bytes: 128,
    });
    cache.insert("small".into(), vec![1]);
    cache.insert("large".into(), vec![0; 200]);
    assert_eq!(cache.stats().entries, 1);
    assert_eq!(cache.stats().rejected, 1);
    assert_eq!(cache.stats().evictions, 0);
    assert_eq!(cache.get("small"), Some(vec![1]));
}

#[test]
fn disabling_cache_and_malformed_payloads_fail_as_misses() {
    let disabled = MemoryCaches::new(MemoryCacheConfig::disabled());
    disabled.insert(true, "key".into(), &vec![1.0_f32, 0.0]);
    assert!(disabled.get::<Vec<f32>>(true, "key").is_none());
    let caches = MemoryCaches::new(MemoryCacheConfig::default());
    caches
        .vectors
        .lock()
        .unwrap()
        .insert("bad".into(), b"invalid-json".to_vec());
    assert!(caches.get::<Vec<f32>>(true, "bad").is_none());
    assert_eq!(caches.stats().query_vectors.entries, 0);
    assert_eq!(caches.stats().query_vectors.invalidations, 1);
}
