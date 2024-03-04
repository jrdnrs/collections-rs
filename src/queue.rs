use core::mem::MaybeUninit;
use std::ops::{Index, IndexMut};

pub struct Queue<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    /// Non-wrapping index of the item to be removed next
    head: usize,
    /// Non-wrapping index of the next available slot
    tail: usize,
    index_mask: usize,
}

impl<T, const N: usize> Queue<T, N> {
    pub fn new() -> Self {
        if !N.is_power_of_two() {
            panic!("Queue size must be a power of two");
        }

        Self {
            data: unsafe { core::mem::MaybeUninit::uninit().assume_init() },
            head: 0,
            tail: 0,
            index_mask: N - 1,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline]
    pub fn clear(&mut self) {
        let (first, second) = self.as_slices_mut();
        let first = first as *mut [T];
        let second = second as *mut [T];

        self.head = 0;
        self.tail = 0;

        // SAFETY:
        // - `first` and `second` are valid pointers to slices of `self.data`.
        // - This might leak `second` if `first` panics (?)
        unsafe {
            core::ptr::drop_in_place(first);
            core::ptr::drop_in_place(second);
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len() {
            return None;
        }

        let index = (self.head + index) & self.index_mask;
        // SAFETY: Due to mask, index is always in bounds
        Some(unsafe { self.data.get_unchecked(index).assume_init_ref() })
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        let index = (self.head + index) & self.index_mask;
        unsafe { self.data.get_unchecked(index).assume_init_ref() }
    }

    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len() {
            return None;
        }

        let index = (self.head + index) & self.index_mask;
        // SAFETY: Due to mask, index is always in bounds
        Some(unsafe { self.data.get_unchecked_mut(index).assume_init_mut() })
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        let index = (self.head + index) & self.index_mask;
        unsafe { self.data.get_unchecked_mut(index).assume_init_mut() }
    }

    #[inline]
    pub fn push_back(&mut self, value: T) {
        if self.len() == N {
            panic!("Queue is full");
        }

        let index = self.tail & self.index_mask;
        self.tail = self.tail.wrapping_add(1);

        // SAFETY: Due to mask, index is always in bounds
        unsafe {
            *self.data.get_unchecked_mut(index) = MaybeUninit::new(value);
        }
    }

    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let index = self.head & self.index_mask;
        self.head = self.head.wrapping_add(1);

        // SAFETY:
        // - Due to mask, index is always in bounds
        // - Management of head means it always points to a valid location, as long as the queue is not empty
        Some(unsafe { self.data.get_unchecked_mut(index).assume_init_read() })
    }

    #[inline]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        if self.is_empty() {
            return (&[], &[]);
        }

        let wrapped_head = self.head & self.index_mask;
        let len = self.len();
        let head_len = (N - wrapped_head).min(len);
        let tail_len = len - head_len;

        let first = unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().add(wrapped_head) as *const T, head_len)
        };

        let second =
            unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, tail_len) };

        (first, second)
    }

    #[inline]
    pub fn as_slices_mut(&mut self) -> (&mut [T], &mut [T]) {
        if self.is_empty() {
            return (&mut [], &mut []);
        }

        let wrapped_head = self.head & self.index_mask;
        let len = self.len();
        let head_len = (N - wrapped_head).min(len);
        let tail_len = len - head_len;

        let first = unsafe {
            core::slice::from_raw_parts_mut(
                self.data.as_mut_ptr().add(wrapped_head) as *mut T,
                head_len,
            )
        };

        let second =
            unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, tail_len) };

        (first, second)
    }
}

impl<T, const N: usize> Drop for Queue<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> Default for Queue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Index<usize> for Queue<T, N> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T, const N: usize> IndexMut<usize> for Queue<T, N> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let queue: Queue<u32, 4> = Queue::new();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_push_pop() {
        let mut queue: Queue<u32, 4> = Queue::new();
        assert!(queue.is_empty());
        queue.push_back(1);
        queue.push_back(2);
        queue.push_back(3);
        queue.push_back(4);
        assert_eq!(queue.len(), 4);
        assert_eq!(queue.pop_front(), Some(1));
        assert_eq!(queue.pop_front(), Some(2));
        assert_eq!(queue.pop_front(), Some(3));
        assert_eq!(queue.pop_front(), Some(4));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut queue: Queue<u32, 4> = Queue::new();
        queue.push_back(1);
        queue.push_back(2);
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_not_empty() {
        let mut queue: Queue<u32, 4> = Queue::new();
        queue.push_back(1);
        assert!(!queue.is_empty());
        queue.push_back(2);
        assert!(!queue.is_empty());
        queue.push_back(3);
        assert!(!queue.is_empty());
        queue.push_back(4);
        assert!(!queue.is_empty());
    }

    #[test]
    fn push_empty() {
        let mut queue: Queue<u32, 4> = Queue::new();
        queue.push_back(1);
        assert_eq!(queue.pop_front(), Some(1));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_as_slices() {
        let mut queue: Queue<u32, 4> = Queue::new();
        queue.push_back(1);
        queue.push_back(2);
        queue.push_back(3);
        queue.push_back(4);
        let (first, second) = queue.as_slices();
        assert_eq!(first, &[1, 2, 3, 4]);
        assert_eq!(second, &[]);
        queue.pop_front();
        queue.pop_front();
        let (first, second) = queue.as_slices();
        assert_eq!(first, &[3, 4]);
        assert_eq!(second, &[]);
        queue.push_back(5);
        queue.push_back(6);
        let (first, second) = queue.as_slices();
        assert_eq!(first, &[3, 4]);
        assert_eq!(second, &[5, 6]);
        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
        let (first, second) = queue.as_slices();
        assert_eq!(first, &[]);
        assert_eq!(second, &[]);
    }

    #[test]
    #[should_panic]
    fn not_power_of_two() {
        let _queue: Queue<u32, 3> = Queue::new();
    }

    #[test]
    #[should_panic]
    fn test_push_full() {
        let mut queue: Queue<u32, 4> = Queue::new();
        queue.push_back(1);
        queue.push_back(2);
        queue.push_back(3);
        queue.push_back(4);
        queue.push_back(5);
    }
}
