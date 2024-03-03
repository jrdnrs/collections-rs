use core::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

pub struct FixedVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> FixedVec<T, N> {
    pub fn new() -> Self {
        Self {
            data: core::array::from_fn(|_| MaybeUninit::uninit()),
            len: 0,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// # Panics
    /// Panics if the stack is full
    #[inline]
    pub fn push(&mut self, value: T) {
        if self.len == N {
            panic!("StackVec is full");
        }

        // SAFETY: We know that the length is less than the capacity
        unsafe {
            *self.data.get_unchecked_mut(self.len) = MaybeUninit::new(value);
        }
        self.len += 1;
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;

        // SAFETY: `self.len` here is always less than `N` and at least 0 after decrementing
        Some(unsafe { self.data.get_unchecked_mut(self.len).assume_init_read() })
    }

    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }

        self.len -= 1;

        self.data.swap(index, self.len);

        // SAFETY: `self.len` here is always less than `N` and at least 0 after decrementing
        Some(unsafe { self.data.get_unchecked_mut(self.len).assume_init_read() })
    }

    #[inline]
    pub fn iter(&self) -> core::slice::Iter<T> {
        self.as_slice().iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<T> {
        self.as_mut_slice().iter_mut()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }

        // SAFETY: Confirmed `index` is less than `self.len`
        Some(unsafe { self.data.get_unchecked(index).assume_init_ref() })
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        self.data.get_unchecked(index).assume_init_ref()
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }

        // SAFETY: Confirmed `index` is less than `self.len`
        Some(unsafe { self.data.get_unchecked_mut(index).assume_init_mut() })
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        self.data.get_unchecked_mut(index).assume_init_mut()
    }

    #[inline]
    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }

        // SAFETY: `self.len` is always less than `N` and at least 0 after decrementing
        Some(unsafe { self.data.get_unchecked(self.len - 1).assume_init_ref() })
    }

    #[inline]
    pub unsafe fn last_unchecked(&self) -> &T {
        self.data.get_unchecked(self.len - 1).assume_init_ref()
    }

    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.len == 0 {
            return None;
        }

        // SAFETY: `self.len` is always less than `N` and at least 0 after decrementing
        Some(unsafe { self.data.get_unchecked_mut(self.len - 1).assume_init_mut() })
    }

    #[inline]
    pub unsafe fn last_unchecked_mut(&mut self) -> &mut T {
        self.data.get_unchecked_mut(self.len - 1).assume_init_mut()
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, self.len) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, self.len) }
    }
}

impl<T, const N: usize> Default for FixedVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Index<usize> for FixedVec<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T, const N: usize> IndexMut<usize> for FixedVec<T, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut vec = FixedVec::<u32, 4>::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);

        assert_eq!(vec.len(), 4);
        assert_eq!(vec.pop(), Some(4));
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.pop(), None);
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn test_swap_remove() {
        let mut vec = FixedVec::<u32, 4>::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);

        assert_eq!(vec.len(), 4);
        assert_eq!(vec.swap_remove(0), Some(1));
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.swap_remove(0), Some(4));
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.swap_remove(0), Some(3));
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.swap_remove(0), Some(2));
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.swap_remove(0), None);
        assert_eq!(vec.swap_remove(7), None);
    }

    #[test]
    fn test_clear() {
        let mut vec = FixedVec::<u32, 4>::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);

        assert_eq!(vec.len(), 4);
        vec.clear();
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn test_iter() {
        let mut vec = FixedVec::<u32, 4>::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);

        let mut iter = vec.iter();

        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), Some(&4));
        assert_eq!(iter.next(), None);
    }

    #[test]
    #[should_panic]
    fn test_push_full() {
        let mut vec = FixedVec::<u32, 4>::new();

        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);
        vec.push(5);
    }
}
