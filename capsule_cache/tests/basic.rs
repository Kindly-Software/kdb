use capsule_cache::CapsuleCache;
use std::time::Duration;

#[test]
fn set_get_remove_round_trip() {
    let cache = CapsuleCache::<String>::new();
    cache
        .insert("k".into(), "v".into(), Duration::from_secs(5))
        .unwrap();
    assert_eq!(cache.get(&"k".into()), Some("v".into()));
    assert_eq!(cache.remove(&"k".into()), Some("v".into()));
    assert_eq!(cache.get(&"k".into()), None);
}

#[test]
fn ttl_expiry_expires_entry() {
    let cache = CapsuleCache::<String>::new();
    cache
        .insert("k".into(), "v".into(), Duration::from_millis(10))
        .unwrap();
    let ttl_before = cache.ttl_remaining(&"k".into()).unwrap();
    assert!(ttl_before <= Duration::from_millis(10));
    assert!(ttl_before > Duration::ZERO);
    // Wait long enough for TTL to elapse.
    std::thread::sleep(Duration::from_millis(20));
    cache.evict_expired();
    assert_eq!(cache.get(&"k".into()), None);
    assert_eq!(cache.ttl_remaining(&"k".into()), None);
}

#[test]
fn incr_preserves_ttl_or_sets_default() {
    let cache = CapsuleCache::<String>::new();
    // Missing key -> initializes with delta.
    assert_eq!(cache.incr(&"count".into(), 2).unwrap(), 2);
    // Existing key increments.
    assert_eq!(cache.incr(&"count".into(), 3).unwrap(), 5);
}

#[test]
fn flush_clears_entries() {
    let cache = CapsuleCache::<String>::new();
    cache
        .insert("k".into(), "v".into(), Duration::from_secs(60))
        .unwrap();
    assert_eq!(cache.get(&"k".into()), Some("v".into()));
    assert!(cache.clear_all() >= 1);
    assert_eq!(cache.get(&"k".into()), None);
}
