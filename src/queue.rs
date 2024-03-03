pub struct Queue<T, const N: usize> {
    data: [T; N],
    head: usize,
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
        self.head = 0;
        self.tail = 0;
    }

    #[inline]
    pub fn push_back(&mut self, value: T) {
        if self.len() == N {
            panic!("Queue is full");
        }

        let index = self.tail & self.index_mask;
        self.tail = self.tail.wrapping_add(1);
        unsafe {
            *self.data.as_mut_ptr().add(index) = value;
        }
    }

    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let index = self.head & self.index_mask;
        self.head = self.head.wrapping_add(1);
        Some(unsafe { self.data.as_ptr().add(index).read() })
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
