//! Iterator implementation for AtomicCapsuleMap.

use crate::serializable::BitwiseSerializable;
use crate::shard::{ShardIter, ShardedMap};
use core::hash::Hash;

/// Iterator over AtomicCapsuleMap entries.
///
/// Provides a snapshot view of the map at iteration time.
/// Concurrent modifications may or may not be visible.
pub struct Iter<'a, K, V>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    shards: &'a [ShardedMap<K, V>],
    current_shard: usize,
    current_iter: Option<ShardIter<K, V>>,
}

impl<'a, K, V> Iter<'a, K, V>
where
    K: Hash + Eq + Clone + Ord + BitwiseSerializable,
    V: Clone + BitwiseSerializable,
{
    pub(crate) fn new(shards: &'a [ShardedMap<K, V>]) -> Self {
        let current_iter = if !shards.is_empty() {
            Some(shards[0].iter())
        } else {
            None
        };

        Self {
            shards,
            current_shard: 0,
            current_iter,
        }
    }
}

impl<K, V> Iterator for Iter<'_, K, V>
where
    K: Hash + Eq + Clone + Ord + BitwiseSerializable,
    V: Clone + BitwiseSerializable,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Try current shard iterator
            if let Some(iter) = &mut self.current_iter {
                if let Some(item) = iter.next() {
                    return Some(item);
                }
            }

            // Move to next shard
            self.current_shard += 1;
            if self.current_shard >= self.shards.len() {
                return None;
            }

            self.current_iter = Some(self.shards[self.current_shard].iter());
        }
    }
}
