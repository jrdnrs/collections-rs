#[derive(Clone, Copy)]
enum Index {
    Free,
    Used(usize),
    OutOfBounds,
}

impl Index {
    pub fn unwrap(self) -> usize {
        match self {
            Index::Used(index) => index,
            _ => panic!("Index is not in use"),
        }
    }

    pub unsafe fn unwrap_unchecked(self) -> usize {
        match self {
            Index::Used(index) => index,
            _ => core::hint::unreachable_unchecked(),
        }
    }
}

pub struct SparseMap<T> {
    /// A packed collection of stored items.
    items: Vec<T>,
    /// A packed collection of keys that correspond to each stored item.
    keys: Vec<usize>,
    /// The layer of indirection. Index into this using a key, to get the index for the items/keys collections.
    indices: Vec<Index>,
}

impl<T> SparseMap<T> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            keys: Vec::with_capacity(capacity),
            indices: vec![Index::Free; 8],
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.items.clear();
        self.keys.clear();
        self.indices.clear();
    }

    /// Applies bounds check
    fn get_index(&self, key: usize) -> Index {
        if key >= self.indices.len() {
            Index::OutOfBounds
        } else {
            self.indices[key]
        }
    }

    #[inline]
    pub fn get(&self, key: usize) -> Option<&T> {
        if let Index::Used(index) = self.get_index(key) {
            Some(&self.items[index])
        } else {
            None
        }
    }

    /// # Safety
    /// - The `key` is used as an index into the `indices` collection to provide indirection into
    ///   the `items` collection. Thus, the key must be within bounds of the `indices` collection.
    /// - In addition, the index it resolves to must be within bounds of the `items` collection, which can
    ///   only be guaranteed if you have inserted an item at that index, and have not removed it.
    #[inline]
    pub unsafe fn get_unchecked(&self, key: usize) -> &T {
        debug_assert!(key < self.indices.len(), "Key out of bounds");

        // SAFETY: Deferred to the caller (see above regarding `key` and `index`)
        let index = self.indices.get_unchecked(key).unwrap_unchecked();

        debug_assert!(index < self.items.len(), "Index out of bounds");

        // SAFETY: Deferred to the caller (see above regarding `index` and `items`)
        unsafe { self.items.get_unchecked(index) }
    }

    #[inline]
    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        if let Index::Used(index) = self.get_index(key) {
            Some(&mut self.items[index])
        } else {
            None
        }
    }

    /// # Safety
    /// - The `key` is used as an index into the `indices` collection to provide indirection into
    ///   the `items` collection. Thus, the key must be within bounds of the `indices` collection.
    /// - In addition, the index it resolves to must be within bounds of the `items` collection, which can
    ///   only be guaranteed if you have inserted an item at that index, and have not removed it.
    #[inline]
    pub unsafe fn get_mut_unchecked(&mut self, key: usize) -> &mut T {
        debug_assert!(key < self.indices.len(), "Key out of bounds");

        // SAFETY: Deferred to the caller (see above regarding `key` and `index`)
        let index = self.indices.get_unchecked(key).unwrap_unchecked();

        debug_assert!(index < self.items.len(), "Index out of bounds");

        // SAFETY: Deferred to the caller (see above regarding `index` and `items`)
        unsafe { self.items.get_unchecked_mut(index) }
    }

    #[inline]
    pub fn insert(&mut self, key: usize, item: T) {
        match self.get_index(key) {
            Index::Used(index) => {
                self.items[index] = item;
            }

            Index::Free => {
                self.indices[key] = Index::Used(self.items.len());
                self.items.push(item);
                self.keys.push(key);
            }

            Index::OutOfBounds => {
                self.indices.resize(key * 2, Index::Free);
                self.indices[key] = Index::Used(self.items.len());
                self.items.push(item);
                self.keys.push(key);
            }
        }
    }

    #[inline]
    pub fn remove(&mut self, key: usize) -> Option<T> {
        if let Index::Used(index) = self.get_index(key) {
            let item = self.items.swap_remove(index);
            self.keys.swap_remove(index);

            // update the index for the key that corresponded to the last index buffer item
            // that we just swapped
            self.indices[self.keys[index]] = Index::Used(index);

            Some(item)
        } else {
            None
        }
    }

    #[inline]
    pub fn contains_key(&self, key: usize) -> bool {
        if let Index::Used(_) = self.get_index(key) {
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn values(&self) -> &[T] {
        self.items.as_slice()
    }

    #[inline]
    pub fn values_mut(&mut self) -> &mut [T] {
        self.items.as_mut_slice()
    }

    #[inline]
    pub fn keys(&self) -> &[usize] {
        self.keys.as_slice()
    }
}

impl<T> Default for SparseMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let sparse_set: SparseMap<u32> = SparseMap::new();
        assert!(sparse_set.items.is_empty());
        assert!(sparse_set.keys.is_empty());
    }

    #[test]
    fn test_insert() {
        let mut sparse_set: SparseMap<u32> = SparseMap::new();
        sparse_set.insert(0, 10);
        assert_eq!(sparse_set.items.len(), 1);
        assert_eq!(sparse_set.keys.len(), 1);
        assert_eq!(sparse_set.items[0], 10);

        sparse_set.insert(1, 20);
        assert_eq!(sparse_set.items.len(), 2);
        assert_eq!(sparse_set.keys.len(), 2);
        assert_eq!(sparse_set.items[1], 20);

        sparse_set.insert(0, 30);
        assert_eq!(sparse_set.items.len(), 2);
        assert_eq!(sparse_set.keys.len(), 2);
        assert_eq!(sparse_set.items[0], 30);
    }

    #[test]
    fn test_get() {
        let mut sparse_set: SparseMap<u32> = SparseMap::new();
        sparse_set.insert(0, 10);
        sparse_set.insert(2, 20);

        assert_eq!(sparse_set.get(0), Some(&10));
        assert_eq!(sparse_set.get(1), None);
        assert_eq!(sparse_set.get(2), Some(&20));
    }

    #[test]
    fn test_get_mut() {
        let mut sparse_set: SparseMap<u32> = SparseMap::new();
        sparse_set.insert(0, 10);
        sparse_set.insert(2, 20);

        assert_eq!(sparse_set.get_mut(0), Some(&mut 10));
        assert_eq!(sparse_set.get_mut(1), None);
        assert_eq!(sparse_set.get_mut(2), Some(&mut 20));

        *sparse_set.get_mut(0).unwrap() = 30;
        assert_eq!(sparse_set.get(0), Some(&30));
    }

    #[test]
    fn test_remove() {
        let mut sparse_set: SparseMap<u32> = SparseMap::new();
        sparse_set.insert(0, 10);
        sparse_set.insert(2, 20);
        sparse_set.insert(3, 30);

        assert_eq!(sparse_set.remove(1), None);

        let removed_item = sparse_set.remove(2);
        assert_eq!(removed_item, Some(20));
        assert_eq!(sparse_set.items, vec![10, 30]);
        assert_eq!(sparse_set.keys, vec![0, 3]);
    }

    #[test]
    fn test_contains_key() {
        let mut sparse_set: SparseMap<u32> = SparseMap::new();
        sparse_set.insert(0, 10);
        sparse_set.insert(2, 20);

        assert_eq!(sparse_set.contains_key(0), true);
        assert_eq!(sparse_set.contains_key(1), false);
        assert_eq!(sparse_set.contains_key(2), true);
    }
}
