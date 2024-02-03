use std::collections::VecDeque;

/// The number of bits used for the index portion of the `StoreKey`. The remaining bits are used for
/// the generation portion of the `StoreKey`. This means that the total number of items able to be stored
/// in a `Store` is 2^`INDEX_BITS`.
const INDEX_BITS: u32 = 22;
/// The mask used to extract the index portion of the `StoreKey`.
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;

#[derive(PartialEq, Eq)]
pub struct StoreKey<T> {
    key: u32,
    _marker: std::marker::PhantomData<T>,
}

// Manual impl needed because of PhantomData
impl<T> Copy for StoreKey<T> {}
impl<T> Clone for StoreKey<T> {
    fn clone(&self) -> StoreKey<T> {
        *self
    }
}
impl<T> core::fmt::Debug for StoreKey<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreKey")
            .field("key", &self.key)
            .field("index", &self.index())
            .field("generation", &self.generation())
            .finish()
    }
}

impl<T> StoreKey<T> {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self {
            key: (generation << INDEX_BITS) | index & INDEX_MASK,
            _marker: std::marker::PhantomData,
        }
    }

    #[inline(always)]
    pub fn from_key(key: u32) -> Self {
        Self {
            key,
            _marker: std::marker::PhantomData,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> u32 {
        self.key
    }

    #[inline(always)]
    pub fn index(&self) -> u32 {
        self.key & INDEX_MASK
    }

    #[inline(always)]
    pub fn generation(&self) -> u32 {
        self.key >> INDEX_BITS
    }
}

pub struct Store<T> {
    /// Collection of items. This is accessed using the index portion of the `StoreKey`.
    items: Vec<T>,
    /// Collection of generations. This is accessed using the index portion of the `StoreKey`, and
    /// refers to the generation of the StoreKey that was used to insert the item.
    generations: Vec<u32>,
    /// Collection of free indices. This is used to recycle indices when items are removed.
    free_indices: VecDeque<usize>,
}

impl<T> Store<T> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            free_indices: VecDeque::with_capacity(capacity),
        }
    }

    pub fn get(&self, key: StoreKey<T>) -> Option<&T> {
        let index = key.index() as usize;
        if self.generations[index] == key.generation() {
            Some(&self.items[index])
        } else {
            None
        }
    }

    /// # Safety
    /// - There is no bounds check performed on the index (however, existence of the key implies it is
    /// within bounds).
    /// - More importantly, there is no check that the generation of the key matches the current
    /// generation of the item at the given index.
    pub unsafe fn get_unchecked(&self, key: StoreKey<T>) -> &T {
        let index = key.index() as usize;
        debug_assert_eq!(
            self.generations[index],
            key.generation(),
            "Key generation mismatch"
        );
        debug_assert!(index < self.items.len(), "Index out of bounds");

        // SAFETY: bounds check deferred to caller
        unsafe { self.items.get_unchecked(index) }
    }

    pub fn get_mut(&mut self, key: StoreKey<T>) -> Option<&mut T> {
        let index = key.index() as usize;
        if self.generations[index] == key.generation() {
            Some(&mut self.items[index])
        } else {
            None
        }
    }

    /// # Safety
    /// - There is no bounds check performed on the index (however, existence of the key implies it is
    /// within bounds).
    /// - More importantly, there is no check that the generation of the key matches the current
    /// generation of the item at the given index.
    pub unsafe fn get_mut_unchecked(&mut self, key: StoreKey<T>) -> &mut T {
        let index = key.index() as usize;
        debug_assert_eq!(
            self.generations[index],
            key.generation(),
            "Key generation mismatch"
        );
        debug_assert!(index < self.items.len(), "Index out of bounds");

        // SAFETY: bounds check deferred to caller
        unsafe { self.items.get_unchecked_mut(index) }
    }

    pub fn push(&mut self, item: T) -> StoreKey<T> {
        let index = if let Some(index) = self.free_indices.pop_front() {
            self.items[index] = item;
            index
        } else {
            self.generations.push(0);
            self.items.push(item);
            self.items.len() - 1
        };

        StoreKey::new(index as u32, self.generations[index])
    }

    pub fn set(&mut self, key: StoreKey<T>, item: T) {
        let index = key.index() as usize;
        if self.generations[index] == key.generation() {
            self.items[index] = item;
        }
    }

    pub fn remove(&mut self, key: StoreKey<T>) {
        let index = key.index() as usize;
        if self.generations[index] == key.generation() {
            self.generations[index] += 1;
            self.free_indices.push_back(index);
        }
    }

    pub fn contains_key(&self, key: StoreKey<T>) -> bool {
        let index = key.index() as usize;
        self.generations[index] == key.generation()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (StoreKey<T>, &T)> {
        self.items
            .iter()
            .enumerate()
            .zip(&self.generations)
            .map(|((index, item), generation)| (StoreKey::new(index as u32, *generation), item))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (StoreKey<T>, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .zip(&self.generations)
            .map(|((index, item), generation)| (StoreKey::new(index as u32, *generation), item))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.generations.clear();
        self.free_indices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let store: Store<u32> = Store::new();
        assert!(store.items.is_empty());
        assert!(store.generations.is_empty());
        assert!(store.free_indices.is_empty());
    }

    #[test]
    fn test_insert() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        assert_eq!(store.items.len(), 1);
        assert_eq!(store.generations.len(), 1);
        assert_eq!(store.free_indices.len(), 0);
        assert_eq!(store.items[0], 10);
        assert_eq!(store.generations[0], 0);
        assert_eq!(key.index(), 0);
        assert_eq!(key.generation(), 0);
    }

    #[test]
    fn test_get() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        assert_eq!(store.get(key), Some(&10));
    }

    #[test]
    fn test_get_mut() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        assert_eq!(store.get_mut(key), Some(&mut 10));
    }

    #[test]
    fn test_remove() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        store.remove(key);
        assert_eq!(store.items.len(), 1);
        assert_eq!(store.generations.len(), 1);
        assert_eq!(store.free_indices.len(), 1);
        assert_eq!(store.items[0], 10);
        assert_eq!(store.generations[0], 1);
        assert_eq!(store.free_indices[0], 0);
    }

    #[test]
    fn test_contains_key() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        assert!(store.contains_key(key));
    }

    #[test]
    /// Test that inserting an item, removing it, and then inserting another item reuses the same
    /// index.
    fn test_insert_remove_insert() {
        let mut store: Store<u32> = Store::new();
        let key = store.push(10);
        store.remove(key);
        let key = store.push(20);
        assert_eq!(store.items.len(), 1);
        assert_eq!(store.generations.len(), 1);
        assert_eq!(store.free_indices.len(), 0);
        assert_eq!(store.items[0], 20);
        assert_eq!(store.generations[0], 1);
        assert_eq!(key.index(), 0);
        assert_eq!(key.generation(), 1);
    }

    #[test]
    fn test_insert_insert_remove_remove() {
        let mut store: Store<u32> = Store::new();
        let key1 = store.push(10);
        let key2 = store.push(20);
        store.remove(key1);
        store.remove(key2);
        assert_eq!(store.items.len(), 2);
        assert_eq!(store.generations.len(), 2);
        assert_eq!(store.free_indices.len(), 2);
        assert_eq!(store.items[0], 10);
        assert_eq!(store.generations[0], 1);
        assert_eq!(store.items[1], 20);
        assert_eq!(store.generations[1], 1);
        assert_eq!(store.free_indices[0], 0);
        assert_eq!(store.free_indices[1], 1);
    }
}
