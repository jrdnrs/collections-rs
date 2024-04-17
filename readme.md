Some collection data structures that I use

#### BitSet
Const generic length array of integers used as a bitset

#### ErasedVec
A homogeneous vec without explicit type, you must enforce the type yourself at runtime otherwise bad things will happen

#### ArrayVec
A fixed length array with extra methods, and a length field, to simulate Vec API

#### SparseMap
Uses keys to a sparse vec as layer of indirection to a dense vec

#### Store
Dense vec with reuse of empty slots using generational indices

#### ArrayQueue
Simple queue using a fixed length array

#### SPSC Channel
Lock/Wait-free bounded queue with a single producer/consumer