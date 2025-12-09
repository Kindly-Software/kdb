use capsule_cache::sharded::ShardedCache;
use std::time::Duration;

#[test]
fn sharded_basic_round_trip() {
    let cache = ShardedCache::<String>::new(4, 1024);
    cache
        .insert("k1".into(), "v1".into(), Duration::from_secs(30))
        .unwrap();
    cache
        .insert("k2".into(), "v2".into(), Duration::from_secs(30))
        .unwrap();

    assert_eq!(cache.get(&"k1".into()), Some("v1".into()));
    assert_eq!(cache.get(&"k2".into()), Some("v2".into()));
}
