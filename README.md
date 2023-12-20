# `someday`
[![CI](https://github.com/hinto-janai/someday/actions/workflows/ci.yml/badge.svg)](https://github.com/hinto-janai/someday/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/someday.svg)](https://crates.io/crates/someday) [![docs.rs](https://docs.rs/someday/badge.svg)](https://docs.rs/someday)

Eventually consistent, multi version concurrency.

`someday` is a [multi-version concurrency control](https://en.wikipedia.org/wiki/Multiversion_concurrency_control) primitive.

All [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s receive [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom) [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html)'s of data along with a timestamp.

The single [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) can write [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom) and chooses when to [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) their changes to the readers.

[`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) is atomic and all future readers from that point will be able to see the new data.

Readers who are holding onto old copies of data will be able to continue to do so indefinitely. If needed, they can always acquire a fresh copy of the data using [`head()`](https://docs.rs/someday/latest/someday/struct.Reader.html#method.head), but them holding onto the old [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html)'s will not block the writer from continuing.

## API
`someday`'s API uses [`git`](https://git-scm.com) syntax and semantically does similar actions.

The [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html):
1. Calls [`add()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.add) to add a [`Patch`](https://docs.rs/someday/latest/someday/trait.Apply) to their data
2. Actually executes those changes by [`commit()`](https://docs.rs/someday/latest/someday/struct.Writer.html#commit.add)'ing
3. Can see local or remote (reader) data whenever
4. Can atomically [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) those changes to the [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s
5. Can continue writing without having to wait on [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s

The [`Reader(s)`](struct.Reader.html):
1. Can continually call [`head()`](https://docs.rs/someday/latest/someday/struct.Reader.html#method.head) to cheaply acquire the latest "head" [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html)
2. Can hang onto those [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html) objects forever (although at the peril of memory-usage)
3. Will eventually catch up whenever the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) calls [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push)

## Example
<img src="https://github.com/hinto-janai/someday/assets/101352116/5473432c-ff39-4c0a-9bf2-00bd97f084dd" width="60%"/>

This example shows the typical use case where the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html):
1. Adds some changes
2. Reads their local changes
3. Locks in those changes by calling [`commit()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.commit)
4. Finally reveals those changes to the readers by calling [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push)

and the [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html):
1. Continually reads their latest head [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html) of the current data
2. Eventually catches up when the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) publishes with [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push)

The code:
```rust
use someday::{
	Patch,
	Writer,Reader,
	Commit,CommitRef,
	CommitInfo,PushInfo,
};

// Create a vector.
let v = vec!["a"];

// Create Reader/Writer for the vector `v`.
let (r, mut w) = someday::new(v);

// The readers see the data.
let commit: CommitRef<Vec<&str>> = r.head();
assert_eq!(commit, vec!["a"]);
assert_eq!(commit.timestamp(), 0);

// Writer writes some data, but does not commit.
w.add(Patch::Fn(|w, _| w.push("b")));
// Nothing committed, data still the same everywhere.
let data: &Vec<&str> = w.data();
assert_eq!(*data, vec!["a"]);
// Patches not yet commit:
assert_eq!(w.staged().len(), 1);

// Readers still see old data.
assert_eq!(r.head(), vec!["a"]);

// Writer writes some more data.
w.add(Patch::Fn(|w, _| w.push("c")));
// Readers still see old data.
assert_eq!(r.head(), vec!["a"]);

// Writer commits their patches.
let commit_info: CommitInfo = w.commit();
// The 2 operation were committed locally
// (only the Writer sees them).
assert_eq!(commit_info.patches, 2);

// Readers still see old data.
assert_eq!(r.head(), vec!["a"]);

// Writer finally reveals those
// changes by calling `push()`.
let push_info: PushInfo = w.push();
assert_eq!(push_info.commits, 1);

// Now readers see updates.
let commit: CommitRef<Vec<&str>> = r.head();
assert_eq!(commit, vec!["a", "b", "c"]);
// Each call to `.commit()` added 1 to the timestamp.
assert_eq!(commit.timestamp(), 1);
```

## Lock-free
Readers are [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom) and most of the time [wait-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Wait-freedom).

The writer is [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom), but may block for a bit in worst case scenarios.

When the writer wants to [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) updates to readers, it must:
1. Atomically update a pointer, at which point all _future_ readers will see the new data
2. Re-apply the patches to the old reclaimed data

The old data _can_ be cheaply reclaimed and re-used by the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) if there are no [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s hanging onto old [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html)'s

## Similar
This library is very similar to [`left_right`](https://docs.rs/left-right) which uses 2 copies (left and right) of the same data to allow for high concurrency.

The big difference is that `someday` theoretically allows _infinite_ copies of new data, as long as the readers continue to hold onto the old references.

A convenience that comes from that is that all data lives as long as there is a reader/writer, so there is no `None` returning `.get()` like in `left_right`. In `someday`, if there is a [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html), they can always access data, even if [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) is dropped and vice-versa.

The downside is that there are potentially infinite copies of very similar data.

This is actually a positive in some cases, but has obvious tradeoffs, see below.

## Tradeoffs
If there are old [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s preventing the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) from reclaiming old data, the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) will create a new copy so that it can continue.

In regular read/write/mutex locks, this is where `lock()`/`write()`/`read()` would hang waiting to acquire the lock.

In [`left_right`](https://docs.rs/left-right), this is where the [`publish()`](https://docs.rs/left-right/0.11.5/left_right/struct.WriteHandle.html#method.publish) function would hang, waiting for all old readers to evacuate.

In `someday`, if the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) cannot reclaim old data, instead of waiting, it will completely clone the data to continue.

This means old [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s are allowed to hold onto old [`Commit`](https://docs.rs/someday/latest/someday/struct.CommitRef.html)'s indefinitely and will **never block the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html).**

This is great for small data structures that aren't too expensive to clone and/or when your [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s are holding onto the data for a while.

The obvious downside is that the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) will _fully clone_ the data over and over again. Depending on how heavy your data is (and if it is de-duplicated via `Arc`, `Rc`, etc) this may take a while.

As the same with `left_right`, `someday` retains all the same downsides:

- **Increased memory use:** The [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) keeps two copies of the backing data structure, and [`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s can keep an infinite amount (although this is actually wanted in some cases)

- **Deterministic patches:** The patches applied to your data must be deterministic, since the [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) must apply them twice

- **Single writer:** There is only a single [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html). To have multiple [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html)'s, you need to ensure exclusive access with something like a `Mutex`

- **Slow writes:** Writes are slower than they would be directly against the backing data structure

- **Patches must be enumerated:** You must define the patches that can be applied to your data and _how_ they apply to your data

## Use-case
`someday` is useful in situations where:

Your data:
- Is relatively cheap to clone (or de-duplicated)

and if you have readers who:
- Want to acquire the latest copy of data, lock-free
- Hold onto data for a little while (or forever)

and a writer that:
- Wants to make changes to data, lock-free
- Wants to "publish" those changes ASAP to new readers, lock-free
- Doesn't need to "publish" data at an extremely fast rate (e.g, 100,000 times a second)
