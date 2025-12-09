//! Entry API for AtomicCapsuleMap.
//!
//! Provides HashMap-like entry API for efficient conditional operations.

use core::hash::Hash;

/// A view into a single entry in the map.
///
/// Allows for in-place manipulation of the value.
pub enum Entry<'a, K, V> {
    /// An occupied entry
    Occupied(OccupiedEntry<'a, K, V>),
    /// A vacant entry
    Vacant(VacantEntry<'a, K, V>),
}

impl<K, V> Entry<'_, K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Ensures a value is in the entry by inserting the default if empty.
    ///
    /// Returns a mutable reference to the value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use atomic_capsule_map::AtomicCapsuleMap;
    ///
    /// let map = AtomicCapsuleMap::new();
    /// map.entry("key").or_insert(42);
    /// ```
    pub fn or_insert(self, default: V) -> V {
        match self {
            Entry::Occupied(entry) => entry.get(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the function if empty.
    pub fn or_insert_with<F>(self, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        match self {
            Entry::Occupied(entry) => entry.get(),
            Entry::Vacant(entry) => entry.insert(f()),
        }
    }

    /// Returns the entry's key.
    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(entry) => entry.key(),
            Entry::Vacant(entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Entry::Occupied(mut entry) => {
                entry.modify(f);
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

/// A view into an occupied entry in the map.
pub struct OccupiedEntry<'a, K, V> {
    _phantom: core::marker::PhantomData<&'a (K, V)>,
}

impl<K: Clone, V: Clone> OccupiedEntry<'_, K, V> {
    /// Gets a copy of the key in the entry.
    pub fn key(&self) -> &K {
        unimplemented!("Architecture Expert should implement")
    }

    /// Gets a copy of the value in the entry.
    pub fn get(&self) -> V {
        unimplemented!("Architecture Expert should implement")
    }

    /// Sets the value in the entry, returning the old value.
    pub fn insert(&mut self, _value: V) -> V {
        unimplemented!("Architecture Expert should implement")
    }

    /// Removes the entry from the map, returning the value.
    pub fn remove(self) -> V {
        unimplemented!("Architecture Expert should implement")
    }

    /// Modifies the value in the entry.
    pub fn modify<F>(&mut self, _f: F)
    where
        F: FnOnce(&mut V),
    {
        unimplemented!("Architecture Expert should implement")
    }
}

/// A view into a vacant entry in the map.
pub struct VacantEntry<'a, K, V> {
    _phantom: core::marker::PhantomData<&'a (K, V)>,
}

impl<K: Clone, V: Clone> VacantEntry<'_, K, V> {
    /// Gets a reference to the key that would be used when inserting.
    pub fn key(&self) -> &K {
        unimplemented!("Architecture Expert should implement")
    }

    /// Sets the value of the entry with the VacantEntry's key.
    pub fn insert(self, _value: V) -> V {
        unimplemented!("Architecture Expert should implement")
    }
}
