use core::{
    cell::Cell,
    mem::size_of,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::{alloc, sync::Arc};

const USIZE: usize = size_of::<AtomicUsize>();
const CACHE_LINE: usize = 64;

fn array_layout<T>(capacity: usize) -> alloc::Layout {
    #[inline(never)]
    fn capacity_overflow() -> ! {
        panic!("capacity overflow");
    }

    match alloc::Layout::array::<T>(capacity) {
        Ok(l) => {
            if usize::BITS < 64 && l.size() > isize::MAX as usize {
                capacity_overflow()
            }
            l
        }
        Err(_) => capacity_overflow(),
    }
}

fn allocate<T>(capacity: usize) -> NonNull<T> {
    if size_of::<T>() == 0 || capacity == 0 {
        return NonNull::dangling();
    }

    let layout = array_layout::<T>(capacity);

    NonNull::new(unsafe { alloc::alloc(layout) })
        .unwrap_or_else(|| alloc::handle_alloc_error(layout))
        .cast()
}

#[repr(C, align(64))]
struct SyncQueue<T, const N: usize> {
    // Consumer cache line
    head: AtomicUsize,
    tail_cache: Cell<usize>,
    _pad1: [u8; CACHE_LINE - 2 * USIZE],

    // Producer cache line
    tail: AtomicUsize,
    head_cache: Cell<usize>,
    _pad2: [u8; CACHE_LINE - 2 * USIZE],

    buffer: NonNull<T>,
}

impl<T, const N: usize> SyncQueue<T, N> {
    fn new() -> Self {
        assert!(N > 0, "capacity must be greater than zero");

        Self {
            head: AtomicUsize::default(),
            tail_cache: Cell::default(),
            _pad1: [0; CACHE_LINE - 2 * USIZE],

            tail: AtomicUsize::default(),
            head_cache: Cell::default(),
            _pad2: [0; CACHE_LINE - 2 * USIZE],

            buffer: allocate(N),
        }
    }

    fn len(&self) -> usize {
        self.tail.load(Ordering::Acquire) - self.head.load(Ordering::Acquire)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn is_full(&self) -> bool {
        self.len() >= N
    }

    fn push(&self, value: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);

        // The buffer cannot increase in size by any other means, so we can keep pushing safely
        // until we reach our cached head index (plus buffer capacity)
        if tail == (self.head_cache.get() + N) {
            let head = self.head.load(Ordering::Acquire);
            self.head_cache.set(head);

            if tail == (head + N) {
                return Err(value);
            }
        }

        // SAFETY:
        // - Due to modulus, index is always in bounds
        // - There is space, according to current head
        unsafe {
            self.buffer
                .as_ptr()
                .offset((tail % N) as isize)
                .write(value);
        }

        self.tail.store(tail + 1, Ordering::Release);
        Ok(())
    }

    fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);

        // The buffer cannot decrease in size by any other means, so we can keep popping safely
        // until we reach our cached tail index
        if head == self.tail_cache.get() {
            let tail = self.tail.load(Ordering::Acquire);
            self.tail_cache.set(tail);

            if head == tail {
                return None;
            }
        }

        // SAFETY:
        // - Due to modulus, index is always in bounds
        // - There is at least one element, according to current tail
        let value = unsafe { self.buffer.as_ptr().offset((head % N) as isize).read() };

        self.head.store(head + 1, Ordering::Release);
        Some(value)
    }
}

impl<T, const N: usize> Drop for SyncQueue<T, N> {
    fn drop(&mut self) {
        if size_of::<T>() == 0 {
            return;
        }

        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        for i in head..tail {
            let i = i % N;
            unsafe { self.buffer.as_ptr().offset(i as isize).drop_in_place() };
        }

        unsafe { alloc::dealloc(self.buffer.cast().as_ptr(), array_layout::<T>(N)) };
    }
}

unsafe impl<T: Sync, const N: usize> Sync for SyncQueue<T, N> {}
unsafe impl<T: Send, const N: usize> Send for SyncQueue<T, N> {}

pub struct Sender<T, const N: usize> {
    buffer: Arc<SyncQueue<T, N>>,
}

impl<T, const N: usize> Sender<T, N> {
    pub fn try_send(&mut self, value: T) -> Result<(), T> {
        self.buffer.push(value)
    }

    pub fn send(&mut self, mut value: T) {
        loop {
            match self.try_send(value) {
                Ok(()) => return,
                Err(v) => value = v,
            }
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.is_full()
    }
}

pub struct Receiver<T, const N: usize> {
    buffer: Arc<SyncQueue<T, N>>,
}

impl<T, const N: usize> Receiver<T, N> {
    pub fn try_receive(&mut self) -> Option<T> {
        self.buffer.pop()
    }

    pub fn receive(&mut self) -> T {
        loop {
            if let Some(value) = self.try_receive() {
                return value;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.is_full()
    }
}

pub fn spsc_channel<T, const N: usize>() -> (Sender<T, N>, Receiver<T, N>) {
    let buffer = Arc::new(SyncQueue::new());

    (
        Sender {
            buffer: buffer.clone(),
        },
        Receiver { buffer },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct XORShift64 {
        state: u64,
    }

    impl XORShift64 {
        pub fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        pub fn rand(&mut self) -> u64 {
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.state = x;
            return x;
        }
    }

    #[derive(Debug, PartialEq)]
    struct BitPattern<const N: usize> {
        bits: [u8; N],
    }

    impl<const N: usize> BitPattern<N> {
        fn new(seed: usize) -> Self {
            let mut rng = XORShift64::new(seed as u64);
            let bits = core::array::from_fn(|_| rng.rand() as u8);
            Self { bits }
        }
    }

    #[test]
    fn try_send_receive() {
        let (mut tx, mut rx) = spsc_channel::<usize, 16>();
        assert_eq!(tx.try_send(42), Ok(()));
        assert_eq!(rx.try_receive(), Some(42));

        assert_eq!(rx.try_receive(), None);

        assert_eq!(tx.try_send(43), Ok(()));
        assert_eq!(tx.try_send(44), Ok(()));
        assert_eq!(rx.try_receive(), Some(43));
        assert_eq!(rx.try_receive(), Some(44));

        assert_eq!(rx.try_receive(), None);
    }

    #[test]
    fn send_receive() {
        let (mut tx, mut rx) = spsc_channel::<usize, 16>();
        tx.send(42);
        assert_eq!(rx.receive(), 42);
    }

    #[test]
    fn full() {
        let (mut tx, rx) = spsc_channel::<usize, 4>();
        assert_eq!(tx.try_send(42), Ok(()));
        assert_eq!(tx.try_send(43), Ok(()));
        assert_eq!(tx.try_send(44), Ok(()));
        assert_eq!(tx.try_send(45), Ok(()));

        assert_eq!(tx.try_send(46), Err(46));
    }

    #[test]
    fn zst() {
        let (mut tx, mut rx) = spsc_channel::<(), 16>();
        tx.send(());
        assert_eq!(rx.receive(), ());
    }

    #[should_panic]
    #[test]
    fn zero_capacity() {
        let (mut tx, mut rx) = spsc_channel::<usize, 0>();
    }

    #[test]
    fn threaded() {
        const ITERS: usize = 1_000_000;

        let (mut tx, mut rx) = spsc_channel::<usize, 256>();

        let t1 = std::thread::spawn(move || {
            for i in 0..ITERS {
                tx.send(i)
            }
        });

        let t2 = std::thread::spawn(move || {
            for i in 0..ITERS {
                assert_eq!(rx.receive(), i);
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    fn threaded_large_type() {
        const ITERS: usize = 10_000;
        const SIZE: usize = 1024 * 10;

        let (mut tx, mut rx) = spsc_channel::<BitPattern<SIZE>, 256>();

        let t1 = std::thread::spawn(move || {
            for i in 0..ITERS {
                tx.send(BitPattern::new(i))
            }
        });

        let t2 = std::thread::spawn(move || {
            for i in 0..ITERS {
                assert_eq!(rx.receive(), BitPattern::new(i));
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    fn threaded_one_capacity() {
        const ITERS: usize = 100_000;
        const SIZE: usize = 256;

        let (mut tx, mut rx) = spsc_channel::<BitPattern<SIZE>, 1>();

        let t1 = std::thread::spawn(move || {
            for i in 0..ITERS {
                tx.send(BitPattern::new(i))
            }
        });

        let t2 = std::thread::spawn(move || {
            for i in 0..ITERS {
                assert_eq!(rx.receive(), BitPattern::new(i));
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();
    }
}
