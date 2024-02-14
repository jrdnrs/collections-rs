use core::{alloc::Layout, any::TypeId, cell::UnsafeCell, marker::PhantomData, ptr::NonNull};
use std::alloc;

const DEFAULT_CAPACITY: usize = 8;

/// This is a wrapper around a [NonNull] pointer, for the sake of associating a lifetime, as well as
/// providing some convenience methods.
///
/// # Safety
/// Attempting to coerce creation of this via a shared reference may result in undefined behavior, if
/// mutation of the underlying data is attempted. For more info, refer to
/// [StackedBorrows](https://github.com/rust-lang/unsafe-code-guidelines/blob/master/wip/stacked-borrows.md).
#[derive(Clone, Copy)]
pub struct Ptr<'a> {
    inner: NonNull<u8>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Ptr<'a> {
    #[inline]
    pub fn new(inner: NonNull<u8>) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn as_ptr(self) -> *mut u8 {
        self.inner.as_ptr()
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The pointer is aligned to the type `T`
    #[inline]
    pub unsafe fn as_ref<T>(self) -> &'a T {
        &*(self.inner.as_ptr() as *const T)
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The pointer is aligned to the type `T`
    #[inline]
    pub unsafe fn as_mut<T>(self) -> &'a mut T {
        &mut *(self.inner.as_ptr() as *mut T)
    }

    #[inline]
    pub fn is_aligned<T>(&self) -> bool {
        self.inner
            .as_ptr()
            .cast::<T>()
            .align_offset(core::mem::align_of::<T>())
            == 0
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The pointer is aligned to the type `T`
    /// - The data that the pointer points to is valid when interpreted as `T`, as the associated
    ///   destructor will be called.
    #[inline]
    pub unsafe fn drop_as<T>(self) {
        debug_assert!(self.is_aligned::<T>());

        self.as_ptr().cast::<T>().drop_in_place();
    }
}

impl<'a, T> From<NonNull<T>> for Ptr<'a> {
    #[inline]
    fn from(inner: NonNull<T>) -> Self {
        Self::new(inner.cast::<u8>())
    }
}

impl<'a, T> From<&'a mut T> for Ptr<'a> {
    #[inline]
    fn from(inner: &'a mut T) -> Self {
        Self::new(NonNull::from(inner).cast::<u8>())
    }
}

impl<'a, T> From<*mut T> for Ptr<'a> {
    #[inline]
    fn from(inner: *mut T) -> Self {
        Self::new(NonNull::new(inner.cast::<u8>()).expect("Null pointer"))
    }
}

#[derive(Clone)]
pub struct ErasedType {
    type_id: TypeId,
    layout: Layout,
    drop: unsafe fn(Ptr),
}

impl ErasedType {
    pub fn new<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            drop: |ptr| unsafe { ptr.drop_as::<T>() },
        }
    }

    pub fn from_raw_parts(type_id: TypeId, layout: Layout, drop: unsafe fn(Ptr)) -> Self {
        Self {
            type_id,
            layout,
            drop,
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - Any aliases of this pointer are not used after calling this function.
    #[inline]
    pub unsafe fn dispose(&self, ptr: Ptr) {
        (self.drop)(ptr);
    }
}

/// A type-erased vector that can store any type.
///
/// Almost every method on this type is unsafe
pub struct ErasedVec {
    item: ErasedType,
    layout: Layout,
    head: NonNull<u8>,
    /// The number of elements in the vec
    len: usize,
    /// The number of elements that can be stored in the vec
    capacity: usize,
}

impl ErasedVec {
    #[inline]
    pub fn new<T: 'static>() -> Self {
        Self::with_capacity::<T>(DEFAULT_CAPACITY)
    }

    #[inline]
    pub fn with_capacity<T: 'static>(capacity: usize) -> Self {
        let item = ErasedType::new::<T>();

        Self::with_capacity_erased_type(item, capacity)
    }

    #[inline]
    pub fn from_erased_type(item: ErasedType) -> Self {
        Self::with_capacity_erased_type(item, DEFAULT_CAPACITY)
    }

    #[inline]
    pub fn with_capacity_erased_type(item: ErasedType, capacity: usize) -> Self {
        // TODO: ZST support
        if capacity == 0 {
            panic!("Capacity must be greater than 0");
        }

        let layout =
            Layout::from_size_align(item.layout.size() * capacity, item.layout.align()).unwrap();

        let head = NonNull::new(unsafe { alloc::alloc(layout) })
            .unwrap_or_else(|| alloc::handle_alloc_error(layout));

        Self {
            item,
            layout,
            head,
            len: 0,
            capacity,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn erased_type(&self) -> &ErasedType {
        &self.item
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The pointer is aligned to the type that this vec was created with.
    /// - The pointer is actually valid for reading a value of the type.
    /// - The pointer should not, for some reason, represent the end (len-wise) of this vec.
    #[inline]
    pub unsafe fn push(&mut self, value: Ptr) {
        // TODO: ZST support
        self.reserve(1);

        // SAFETY:
        // - Dst pointer is guaranteed to be aligned, as it is derived from the allocation pointer, and
        //   is incremented by element's size.
        // - Pointers are derived from [NonNull] pointers, so they are guaranteed to be non-null.
        // - Howewer, we are deferring the check for validity of the src pointer to the caller.
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                self.get_unchecked(self.len).as_ptr(),
                self.item.layout.size(),
            )
        }

        self.len += 1;
    }

    #[inline]
    pub unsafe fn push_many(&mut self, values: Ptr, count: usize) {
        // TODO: ZST support
        self.reserve(count);

        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                self.get_unchecked(self.len).as_ptr(),
                self.item.layout.size(),
            )
        }

        self.len += count;
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The data associated with this pointer is **not** dropped, as the vec will continue to hold a reference
    ///   to it.
    #[inline]
    pub unsafe fn get(&self, index: usize) -> Option<Ptr> {
        if index < self.len {
            // SAFETY: self.len is within bounds
            Some(unsafe { self.get_unchecked(index) })
        } else {
            None
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The data associated with this pointer is **not** dropped, as the vec will continue to hold a reference
    ///   to it.
    /// - The index is within the bounds of the vec.
    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> Ptr {
        // SAFETY: Bounds check deferred to the caller.
        unsafe { Ptr::from(self.head.as_ptr().add(index * self.item.layout.size())) }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The data associated with this pointer **is** dropped appropriately if necessary, by calling `dispose`
    ///   on the vec. Not doing so will potentially leak memory, as the vec will no longer track this item.
    #[inline]
    pub unsafe fn pop(&mut self) -> Option<Ptr> {
        if self.len > 0 {
            self.len -= 1;
            // Safety: `self.len` is within bounds
            unsafe { Some(self.get_unchecked(self.len)) }
        } else {
            None
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The data associated with this pointer **is** dropped appropriately if necessary, by calling `dispose`
    ///   on the vec. Not doing so will potentially leak memory, as the vec will no longer track this item.
    #[inline]
    pub unsafe fn pop_many(&mut self, count: usize) -> Option<Ptr> {
        if self.len >= count {
            self.len -= count;
            // Safety: `self.len` is within bounds
            unsafe { Some(self.get_unchecked(self.len)) }
        } else {
            None
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The vec is not empty, as no bounds checking is done.
    /// - The data associated with this pointer **is** dropped appropriately if necessary, by calling `dispose`
    ///   on the vec. Not doing so will potentially leak memory, as the vec will no longer track this item.
    #[inline]
    pub unsafe fn pop_unchecked(&mut self) -> Ptr {
        self.len -= 1;
        // SAFETY: Just decremented self.len, so this is fine as it effectively gets the last element.
        unsafe { self.get_unchecked(self.len) }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The data associated with this pointer **is** dropped appropriately if necessary, by calling `dispose`
    ///   on the vec. Not doing so will potentially leak memory, as the vec will no longer track this item.
    #[inline]
    pub unsafe fn swap_remove(&mut self, index: usize) -> Option<Ptr> {
        if index < self.len {
            // SAFETY: index is within bounds
            Some(unsafe { self.swap_remove_unchecked(index) })
        } else {
            None
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The vec is not empty, as no bounds checking is done.
    /// - The index is within the bounds of the vec.
    /// - The data associated with this pointer **is** dropped appropriately if necessary, by calling `dispose`
    ///   on the vec. Not doing so will potentially leak memory, as the vec will no longer track this item.
    #[inline]
    pub unsafe fn swap_remove_unchecked(&mut self, index: usize) -> Ptr {
        // TODO: think of better way to handle potential swap with self
        if index == self.len - 1 {
            // SAFETY: Confirmed that len is at least 1.
            return unsafe { self.pop_unchecked() };
        }

        self.len -= 1;

        // SAFETY: Just decremented self.len, so this is fine as it effectively gets the last element.
        let end = unsafe { self.get_unchecked(self.len) };
        // SAFETY: Bounds check deferred to the caller.
        let middle = unsafe { self.get_unchecked(index) };

        debug_assert_ne!(end.as_ptr(), middle.as_ptr());

        // SAFETY:
        // - `middle` and `end` pointers are different and, as they vary by increments of one element's size,
        //   they are guaranteed to not overlap.
        // - By virtue of incrementing by one element's size, the pointers are guaranteed to be aligned.
        // - They are derived from [NonNull] pointers, so they are guaranteed to be non-null.
        // - As long as the caller ensures that `index` is within bounds, the pointers are guaranteed to
        //   point to valid memory.
        unsafe {
            core::ptr::swap_nonoverlapping(end.as_ptr(), middle.as_ptr(), self.item.layout.size())
        };

        end
    }

    /// # Safety
    /// The caller must ensure that:
    /// - Any existing pointers to the data are not used after this.
    #[inline]
    pub unsafe fn clear(&mut self) {
        for i in 0..self.len {
            // SAFETY: `self.len` is within bounds
            let ptr = unsafe { self.get_unchecked(i) };
            // SAFETY: `ptr` is not used after this call
            unsafe { self.item.dispose(ptr) }
        }

        self.len = 0;
    }

    /// # Safety
    /// The caller must ensure that:
    /// - 'T' actually has the same size and alignment as the item type of this vec.
    #[inline]
    pub unsafe fn as_slice<T>(&self) -> &[UnsafeCell<T>] {
        unsafe { core::slice::from_raw_parts(self.head.as_ptr().cast::<UnsafeCell<T>>(), self.len) }
    }

    #[inline]
    unsafe fn reserve(&mut self, additional: usize) {
        let required = self.len + additional;
        if required > self.capacity {
            self.grow(required.next_power_of_two());
        }
    }

    /// # Safety
    /// The caller must ensure that:
    /// - The item size and capacity are greater than zero (ZST)
    /// - The new capacity is greater than the current capacity.
    unsafe fn grow(&mut self, new_capacity: usize) {
        self.capacity = new_capacity;

        let new_layout = Layout::from_size_align(
            self.item.layout.size() * self.capacity,
            self.item.layout.align(),
        )
        .expect("Invalid layout");

        // SAFETY:
        // - self.data` is guaranteed to be non-null.
        // -`self.data_layout` is valid, otherwise we will have already panicked.
        let new_head =
            unsafe { alloc::realloc(self.head.as_ptr(), self.layout, new_layout.size()) };

        self.layout = new_layout;

        self.head =
            NonNull::new(new_head).unwrap_or_else(|| alloc::handle_alloc_error(self.layout));
    }
}

impl Drop for ErasedVec {
    fn drop(&mut self) {
        unsafe { self.clear() };

        // TODO: ZST support (no need to deallocate if size is zero)
        unsafe {
            alloc::dealloc(self.head.as_ptr(), self.layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use core::mem::ManuallyDrop;

    use super::*;

    #[test]
    fn drop_test() {
        unsafe { _drop_test() }
    }

    unsafe fn _drop_test() {
        let mut vec = ErasedVec::new::<String>();
        let mut element = ManuallyDrop::new(String::from("Hello World"));
        vec.push(Ptr::from(&mut element));

        assert_eq!(vec.len(), 1);
        assert_eq!(vec.capacity(), 8);

        // 'removed' from vec, but still lives there so we are responsible for dropping it.
        // ... also, this mess of method chaining is to coerce the associated lifetime so I can continue
        // working on the vec for testing purposes.
        let element = vec
            .pop()
            .unwrap()
            .as_ptr()
            .cast::<String>()
            .as_mut()
            .unwrap();
        println!("1: {}", element);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 8);

        // element still not dropped, because vec doesn't know about it
        vec.clear();
        println!("2: {}", element);

        // drop the element using vec
        vec.item.dispose(Ptr::from(element));
    }

    #[test]
    fn push_pop_test() {
        unsafe { _push_pop_test() }
    }

    unsafe fn _push_pop_test() {
        let mut vec = ErasedVec::new::<i32>();

        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 8);

        for i in 0..10 {
            vec.push(Ptr::from(&i as *const _ as *mut u8));
        }

        assert_eq!(vec.len(), 10);
        assert_eq!(vec.capacity(), 16);

        for i in (0..10).rev() {
            assert_eq!(*vec.pop().unwrap().as_ref::<i32>(), i);
        }

        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 16);

        assert!(vec.pop().is_none());
    }

    #[test]
    fn swap_remove_test() {
        unsafe { _swap_remove_test() }
    }

    unsafe fn _swap_remove_test() {
        let mut vec = ErasedVec::new::<i32>();

        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 8);

        for i in 0..10 {
            vec.push(Ptr::from(&i as *const _ as *mut u8));
        }

        assert_eq!(vec.len(), 10);
        assert_eq!(vec.capacity(), 16);

        // remove the 8th element, which is 7
        assert_eq!(*vec.swap_remove(7).unwrap().as_ref::<i32>(), 7);
        // removed the last element (9th), which was 9 but was swapped with 7, so now 8
        assert_eq!(*vec.pop().unwrap().as_ref::<i32>(), 8);

        // 2 elements removed so far
        assert_eq!(vec.len(), 8);

        // remove the last element (8th), which now should be 9 as it was swapped with 7
        assert_eq!(*vec.pop().unwrap().as_ref::<i32>(), 9);
        // remove the last element (7th), which now should be 6 as normal
        assert_eq!(*vec.pop().unwrap().as_ref::<i32>(), 6);

        // 4 elements removed so far
        assert_eq!(vec.len(), 6);

        // remove all but one
        for i in (1..6).rev() {
            vec.swap_remove_unchecked(i);
        }
        assert_eq!(vec.len(), 1);

        // try to swap remove the last element with itself
        assert_eq!(*vec.swap_remove_unchecked(0).as_ref::<i32>(), 0);
    }

    #[test]
    fn as_slice_test() {
        unsafe { _as_slice_test() }
    }

    unsafe fn _as_slice_test() {
        let mut vec = ErasedVec::new::<i32>();

        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 8);

        for i in 0..10 {
            vec.push(Ptr::from(&i as *const _ as *mut u8));
        }

        assert_eq!(vec.len(), 10);
        assert_eq!(vec.capacity(), 16);

        let slice = vec.as_slice::<i32>();

        assert_eq!(slice.len(), 10);

        for i in 0..10 {
            assert_eq!(*slice[i].get(), i as i32);
        }
    }
}
