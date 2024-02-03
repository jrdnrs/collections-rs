use core::{
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not},
    slice::Iter,
};

// u64 allows for automatic SIMD vectorization on x86_64, but u128 is faster for
// leading/trailing zeros (?). We could coerce compiler to use u64 for SIMD via transmute,
// but not sure how to do that with const generics.
const DEFAULT_CAPACITY: usize = 1;
const BITS_PER_ELEMENT: usize = 64;
type Element = u64;

/// A bitset with a fixed length, configurable via const generics where `L` is the number of `Element`s
/// used to store the bits.
///
/// Defaults to 1 `Element`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BitSet<const L: usize = DEFAULT_CAPACITY> {
    bits: [Element; L],
}

impl<const L: usize> BitSet<L> {
    pub fn new() -> Self {
        Self { bits: [0; L] }
    }

    pub fn from_index(index: usize) -> Self {
        let mut bit_set = Self::new();
        bit_set.set(index);
        return bit_set;
    }

    pub fn set(&mut self, index: usize) {
        let (i, j) = (index / BITS_PER_ELEMENT, index % BITS_PER_ELEMENT);
        self.bits[i] |= 1 << j;
    }

    pub fn clear(&mut self, index: usize) {
        let (i, j) = (index / BITS_PER_ELEMENT, index % BITS_PER_ELEMENT);
        self.bits[i] &= !(1 << j);
    }

    pub fn test(&self, index: usize) -> bool {
        let (i, j) = (index / BITS_PER_ELEMENT, index % BITS_PER_ELEMENT);
        self.bits[i] & (1 << j) != 0
    }

    /// Returns true if other is a subset of self
    pub fn contains(&self, other: &Self) -> bool {
        self | other == *self
    }

    pub fn contains_none(&self, other: &Self) -> bool {
        (self & other).is_empty()
    }

    pub fn contains_some(&self, other: &Self) -> bool {
        !(self & other).is_empty()
    }

    /// Returns bits that are in self, without bits in other
    pub fn difference(&self, other: &Self) -> Self {
        self & &!other
    }

    /// Returns bits that are in self or in other, but not in both
    pub fn symmetric_difference(&self, other: &Self) -> Self {
        self ^ other
    }

    /// Returns bits that are in self and other
    pub fn intersection(&self, other: &Self) -> Self {
        self & other
    }

    /// Returns bits that are in self and/or other
    pub fn union(&self, other: &Self) -> Self {
        self | other
    }

    pub fn leading_zeros(&self) -> usize {
        let mut result = 0;
        for bits in self.bits.iter().rev() {
            result += bits.leading_zeros() as usize;

            if *bits > 0 {
                break;
            }
        }
        return result;
    }

    pub fn trailing_zeros(&self) -> usize {
        let mut total = 0;
        for bits in self.bits.iter() {
            total += bits.trailing_zeros() as usize;

            if *bits > 0 {
                break;
            }
        }
        return total;
    }

    pub fn count_ones(&self) -> usize {
        let mut total = 0;
        for bits in self.bits.iter() {
            total += bits.count_ones() as usize;
        }
        return total;
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|bits| *bits == 0)
    }

    pub fn iter_indices(&self) -> SetBitsIter<L> {
        let mut bit_slices = self.bits.iter();
        let current_bits = *bit_slices.next().unwrap();

        SetBitsIter {
            bit_slices,
            slice_index: 0,
            current_bits,
        }
    }
}

/// Iterator over the indices of a bitset that are set to 1
pub struct SetBitsIter<'a, const L: usize> {
    bit_slices: Iter<'a, Element>,
    slice_index: usize,
    current_bits: Element,
}

impl<'a, const L: usize> Iterator for SetBitsIter<'a, L> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        // skip until we find an element that isn't 0
        while self.current_bits == 0 {
            self.current_bits = *self.bit_slices.next()?;
            self.slice_index += 1;
        }

        let trailing_zeros = self.current_bits.trailing_zeros() as usize;

        // clears the lowest significant bit
        self.current_bits = self.current_bits & self.current_bits.wrapping_sub(1);

        return Some(self.slice_index * BITS_PER_ELEMENT + trailing_zeros);
    }
}

impl<const L: usize> Default for BitSet<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const L: usize> Not for &BitSet<L> {
    type Output = BitSet<L>;

    fn not(self) -> Self::Output {
        let bits = core::array::from_fn(|i| !self.bits[i]);
        BitSet { bits }
    }
}

macro_rules! impl_bitwise_assign {
    ( $trait:ident, $fn:ident, $op:tt ) => {

        impl<const L: usize> $trait<&BitSet<L>> for BitSet<L> {
            fn $fn(&mut self, rhs: &BitSet<L>) {
                for i in 0..L {
                    self.bits[i] $op rhs.bits[i];
                }
            }
        }

        impl<const L: usize> $trait<&mut BitSet<L>> for BitSet<L> {
            fn $fn(&mut self, rhs: &mut BitSet<L>) {
                for i in 0..L {
                    self.bits[i] $op rhs.bits[i];
                }
            }
        }

    };
}

macro_rules! impl_bitwise {
    ( $trait:ident, $fn:ident, $op:tt ) => {

        impl<const L: usize> $trait for &BitSet<L> {
            type Output = BitSet<L>;

            fn $fn(self, rhs: Self) -> Self::Output {
                let bits = core::array::from_fn(|i| self.bits[i] $op rhs.bits[i]);
                BitSet { bits }
            }
        }

    };
}

impl_bitwise!(BitAnd, bitand, &);
impl_bitwise!(BitOr, bitor, |);
impl_bitwise!(BitXor, bitxor, ^);
impl_bitwise_assign!(BitAndAssign, bitand_assign, &=);
impl_bitwise_assign!(BitOrAssign, bitor_assign, |=);
impl_bitwise_assign!(BitXorAssign, bitxor_assign, ^=);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitset() {
        let mut bitset1 = BitSet::<1>::new();
        bitset1.set(17);
        assert!(bitset1.test(17));
        bitset1.clear(17);
        assert!(!bitset1.test(17));

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);
        assert!(bitset2.test(17));
        assert!(bitset2.test(BITS_PER_ELEMENT + 5));
    }

    #[test]
    fn test_bitset_and() {
        let mut bitset1 = BitSet::<2>::new();
        bitset1.set(17);
        bitset1.set(63);

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);

        let result = (&bitset1) & (&bitset2);

        assert!(result.test(17));
        assert!(!result.test(63));
        assert!(!result.test(BITS_PER_ELEMENT + 5));
    }

    #[test]
    fn test_bitset_or() {
        let mut bitset1 = BitSet::<2>::new();
        bitset1.set(17);
        bitset1.set(63);

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);

        let result = (&bitset1) | (&bitset2);

        assert!(result.test(17));
        assert!(result.test(63));
        assert!(result.test(BITS_PER_ELEMENT + 5));
    }

    #[test]
    fn test_bitset_xor() {
        let mut bitset1 = BitSet::<2>::new();
        bitset1.set(17);
        bitset1.set(63);

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);

        let result = (&bitset1) ^ (&bitset2);

        assert!(!result.test(17));
        assert!(result.test(63));
        assert!(result.test(BITS_PER_ELEMENT + 5));
    }

    #[test]
    fn test_bitset_not() {
        let mut bitset1 = BitSet::<2>::new();
        bitset1.set(17);
        bitset1.set(63);

        let result = !(&bitset1);

        assert!(!result.test(17));
        assert!(!result.test(63));
        assert!(result.test(BITS_PER_ELEMENT + 5));
    }

    #[test]
    fn test_bitset_index_iter() {
        let mut bitset1 = BitSet::<2>::new();
        bitset1.set(17);
        bitset1.set(63);
        bitset1.set(BITS_PER_ELEMENT + 5);

        let mut iter = bitset1.iter_indices();
        assert_eq!(iter.next(), Some(17));
        assert_eq!(iter.next(), Some(63));
        assert_eq!(iter.next(), Some(BITS_PER_ELEMENT + 5));
        assert_eq!(iter.next(), None);

        let bitset2 = BitSet::<2>::new();
        let mut iter = bitset2.iter_indices();
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_bitset_trailing_zeros() {
        let mut bitset1 = BitSet::<1>::new();
        bitset1.set(17);
        bitset1.set(63);
        assert_eq!(bitset1.trailing_zeros(), 17);

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);
        assert_eq!(bitset2.trailing_zeros(), 17);
    }

    #[test]
    fn test_bitset_leading_zeros() {
        let mut bitset1 = BitSet::<1>::new();
        bitset1.set(17);
        bitset1.set(63);
        assert_eq!(bitset1.leading_zeros(), BITS_PER_ELEMENT - (63 + 1));

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);
        assert_eq!(bitset2.leading_zeros(), BITS_PER_ELEMENT - (5 + 1));
    }

    #[test]
    fn test_bitset_count_ones() {
        let mut bitset1 = BitSet::<1>::new();
        bitset1.set(17);
        bitset1.set(63);
        assert_eq!(bitset1.count_ones(), 2);

        let mut bitset2 = BitSet::<2>::new();
        bitset2.set(17);
        bitset2.set(BITS_PER_ELEMENT + 5);
        assert_eq!(bitset2.count_ones(), 2);
    }
}
