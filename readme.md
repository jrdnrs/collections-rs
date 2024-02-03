Some collection data structures that I use

#### BitSet
Const generic length array of integers used as a bitset

#### ErasedVec
A homogeneous vec without explicit type, you must enforce the type yourself at runtime otherwise bad things will happen

#### FixedVec
Just a fixed length array with a length field to simulate a vec

#### SparseMap
Uses keys to a sparse vec as layer of indirection to a dense vec

#### Store
Dense vec with reuse of empty slots using generational indices