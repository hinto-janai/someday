# `someday`
[![CI](https://github.com/hinto-janai/someday/actions/workflows/ci.yml/badge.svg)](https://github.com/hinto-janai/someday/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/someday.svg)](https://crates.io/crates/someday) [![docs.rs](https://docs.rs/someday/badge.svg)](https://docs.rs/someday)

Eventually consistent, multi version concurrency.

`someday` is a [multi-version concurrency control](https://en.wikipedia.org/wiki/Multiversion_concurrency_control) primitive that is similar to a standard reader/writer lock but is [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom).

All readers receive [wait-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Wait-freedom) (sometimes lock-free) "snapshots" of data along with a timestamp.

The single writer can write lock-free and chooses when to "publish" their changes to the readers.

The "publish" operation ([`commit()`](https://docs.rs/someday/struct.Writer.html#method.commit)) is atomic and all future readers from that point will be able to see the new data.

Readers who are holding onto old copies of data will be able to continue to do so indefinitely. If needed, they can always acquire a "fresh" copy of the data ([`snapshot()`](https://docs.rs/someday/struct.Reader.html#method.snapshot)) but them holding onto the old copies will not block the writer from continuing.



## Example
<img src="https://github.com/hinto-janai/someday/assets/101352116/b6404356-410a-493c-a101-dc5a189de537" width="50%"/>

This example shows the typical use case where the [`Writer`](https://docs.rs/someday/struct.Writer.html):
1. Applies some changes
2. Reads their local changes
3. Applies some more changes
4. Finally "commits" those changes publically

and the [`Reader`](https://docs.rs/someday/struct.Reader.html):
1. Continually reads "snapshots" of the current data
2. Eventually catches up with the writer and sees the new data when `commit()` is called

The code:
```rust
use someday::ops::{OperationVec,Operation};
use someday::{Writer,Reader,Snapshot};

// Create a vector.
let v = vec!["a"];

// Create Writer/Reader for the vector `v`.
let (mut w, r) = someday::new(v);

// The readers see the data.
let snap: Snapshot<Vec<&str>> = r.snapshot();
assert_eq!(snap, vec!["a"]);
assert_eq!(snap.timestamp(), 0);

// Writer writes some data, but does not commit.
w.apply(OperationVec::Push("b"));
// The writer can see the updated data.
let data: &Vec<&str> = w.read();
assert_eq!(*data, vec!["a", "b"]);

// But readers still see old data.
assert_eq!(r.snapshot(), vec!["a"]);

// Writer writes some more data.
w.apply(OperationVec::Push("c"));
// But readers still see old data.
assert_eq!(r.snapshot(), vec!["a"]);

// Writer commits their operations.
let ops: usize = w.commit();
assert_eq!(ops, 2); // there were 2 operation commited.

// Now readers see updates.
let snap: Snapshot<Vec<&str>> = r.snapshot();
assert_eq!(snap, vec!["a", "b", "c"]);
// Each call to `.apply()` added 1 to the timestamp.
assert_eq!(snap.timestamp(), 2);
```

## Lock-free
Readers are [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom) and most of the time [wait-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Wait-freedom).

The writer is [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom), but may block for a bit in worst case scenarios.

When the writer wants to "publish" updates to readers, it must:
1. Atomically update a pointer, at which point all _future_ readers will see the new data
2. Re-apply the operations that created the new data, _to the old data_

The "old" data _can_ be cheaply reclaimed and re-used by the writer if there are no dangling readers.

## Similar
This library is very similar to [`left_right`](https://docs.rs/left-right) which uses 2 copies (left and right) of the same data to allow for high concurrency.

The big difference is that `someday` can theoretically create _infinite_ copies of new data, as long as the readers continue to hold onto the old references.

A convenience that comes from that is that all data lives as long as there is a reader/writer, so there is no `None` returning `.get()` like in `left_right`. In `someday`, if there is a [`Reader`](https://docs.rs/someday/struct.Reader.html), they can always access data, even if [`Writer`](https://docs.rs/someday/struct.Writer.html) is dropped and vice-versa.

The downside is that there are potentially infinite copies of very similar data.

This is actually a positive in some cases, but has obvious tradeoffs, see below.

## Tradeoffs
If there are old readers preventing the writer from reclaiming old data, the writer will create a new copy so that it can continue, in normal read/write locks, this is where the writer would hang waiting to acquire the lock, in [`left_right`](https://docs.rs/left-right), this is where the [`publish()`](https://docs.rs/left-right/0.11.5/left_right/struct.WriteHandle.html#method.publish) function would hang, waiting for all old readers to evacuate.

In `someday`, if the writer cannot reclaim old data, instead of waiting, it will completely clone the data to continue.

This means old readers are allowed to hold onto old "snapshots" indefinitely and will **never block the writer.**

This is great for small data structures that aren't too expensive to clone and/or when your readers are holding onto the data for a while.

The obvious downside is that the writer will _fully clone_ the data over and over again. Depending on how heavy your data is (and if it is de-duplicated via `Arc`, `Cow`, etc) this may take a while.

As the same with `left_right`, `someday` retains all the same downsides:

- **Increased memory use:** The writer keeps two copies of the backing data structure, and readers can keep an infinite amount (although this is actually wanted in some cases)

- **Deterministic operations:** The operations applied to your data (how it is mutated) must be deterministic, since the writer must apply the same changes twice

- **Single writer:** `someday` has a single writer. To have multiple writers, you need to ensure exclusive access to the through something like a `Mutex`

- **Slow writes:** Writes through `someday` are slower than they would be directly against the backing data structure

## Benefits & Use-cases
- **Lock-free reads of the latest shared data:** Readers never lock to acquire the latest copy of the shared data
- **Multiple hanging readers:** Readers can hold onto data forever and will never block writers
- **Lock-free writer:** Writers will not block if there are no dangling readers, will worse case clone data if there are

If you have readers who:
- Need the latest copy of shared data
- Hold onto it for at least a little while

and a writer that:
- Wants to "publish" changes as quick as possible to new readers

Then `someday` is pretty okay.
