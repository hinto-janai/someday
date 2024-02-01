# `someday`
[![CI](https://github.com/hinto-janai/someday/actions/workflows/ci.yml/badge.svg)](https://github.com/hinto-janai/someday/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/someday.svg)](https://crates.io/crates/someday) [![docs.rs](https://docs.rs/someday/badge.svg)](https://docs.rs/someday)

`someday` is a [multi-version concurrency control](https://en.wikipedia.org/wiki/Multiversion_concurrency_control) primitive.

A lock-free, but more memory hungry alternative to `Mutex<T>` or `RwLock<T>`.

## Lock-free
[`Reader`](https://docs.rs/someday/latest/someday/struct.Reader.html)'s are [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom) and most of the time [wait-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Wait-freedom).

The single [`Writer`](https://docs.rs/someday/latest/someday/struct.Writer.html) is [lock-free](https://en.wikipedia.org/wiki/Non-blocking_algorithm#Lock-freedom), but may clone data.

When the `Writer` wants to [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) updates to `Reader`'s, it must:
1. Atomically update a pointer, at which point all _future_ readers will see the new data
2. Re-apply the patches to the old reclaimed data OR clone the data if it cannot be reclaimed

The old data _can_ be cheaply reclaimed and re-used by the `Writer` if there are no `Reader`'s hanging onto old [`Commit`](https://docs.rs/someday/latest/someday/commit/type.CommitRef.html)'s.

If there are still `Reader`'s hanging onto old data, the `Writer` will clone the data such that it can continue.

## Use-case
`someday` is best in situations where:

Your data:
- Is relatively cheap to clone and/or de-duplicated

and if you have **many** readers who:
- Want to acquire a copy of data, lock-free
- Hold onto data (for a little while or forever)

and a writer that:
- Wants to mutate data, lock-free
- Wants to push changes ASAP to new readers, lock-free
- Doesn't mutate data that often (relative to read operations)
- Is normally in contention with readers using normal locks (`Mutex`, `RwLock`)

## Tradeoffs
- **Increased memory use:** The `Writer` keeps at least two copies of the backing data structure, and `Reader`'s can keep an infinite amount (as long as they continue to hold onto references)

- **Deterministic patches:** The patches/functions applied to your data must be deterministic, since the `Writer` may apply them twice

- **Slow writes:** Writes are slower than they would be directly against the backing data structure

## API
`someday`'s API is similar to [`git`](https://git-scm.com) and semantically does similar actions.

The `Writer`:
1. Calls [`add()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.add) to add a [`Patch`](https://docs.rs/someday/latest/someday/enum.Patch.html) (function) to their data
2. Actually executes those changes by [`commit()`](https://docs.rs/someday/latest/someday/struct.Writer.html#commit.add)'ing
3. Can see local or remote (reader) data whenever
4. Can atomically [`push()`](https://docs.rs/someday/latest/someday/struct.Writer.html#method.push) those changes to the `Reader`'s
5. Can continue writing without having to wait on `Reader`'s

The `Reader(s)`:
1. Can continually call [`head()`](https://docs.rs/someday/latest/someday/struct.Reader.html#method.head) to cheaply acquire the latest "head" `Commit`
2. Can hang onto those `Commit` objects forever (although at the peril of memory-usage)
3. Will eventually catch up whenever the `Writer` calls `push()`

## Example
<img src="https://github.com/hinto-janai/someday/assets/101352116/b190db72-c56b-4336-a601-78296040d044" width="60%"/>

This example shows the typical use case where the `Writer`:
1. Adds some changes
2. Reads their local changes
3. Locks in those changes by calling `commit()`
4. Finally reveals those changes to the readers by calling `push()`

and the `Reader`:
1. Continually reads their latest head `Commit` of the current data
2. Eventually catches up when the `Writer` publishes with `push()`

The code:
```rust
use someday::{
	Patch,
	Writer,Reader,
	Commit,CommitRef,
	CommitInfo,PushInfo,
};

// Create Reader/Writer for the string "hello".
let (r, mut w) = someday::new("hello".to_string());

// The readers see the data.
let commit: CommitRef<String> = r.head();
assert_eq!(commit.data, "hello");
assert_eq!(commit.timestamp, 0);

// Writer writes some data, but does not commit.
w.add(Patch::Ptr(|w, _| w.push_str(" world")));
// Nothing committed, data still the same everywhere.
let data: &String = w.data();
assert_eq!(*data, "hello");
// Patches not yet committed:
assert_eq!(w.staged().len(), 1);

// Readers still see old data.
assert_eq!(r.head().data, "hello");

// Writer writes some more data.
w.add(Patch::Ptr(|w, _| w.push_str("!")));
// Readers still see old data.
assert_eq!(r.head().data, "hello");

// Writer commits their patches.
let commit_info: CommitInfo = w.commit();
// The 2 operation were committed locally
// (only the Writer sees them).
assert_eq!(commit_info.patches, 2);

// Readers still see old data.
assert_eq!(r.head().data, "hello");

// Writer finally reveals those
// changes by calling `push()`.
let push_info: PushInfo = w.push();
assert_eq!(push_info.commits, 1);

// Now readers see updates.
let commit: CommitRef<String> = r.head();
assert_eq!(commit.data, "hello world!");
// Each call to `.commit()` added 1 to the timestamp.
assert_eq!(commit.timestamp, 1);
```

## Feature Flags
These features are for (de)serialization.

You can directly (de)serialize your data `T` from a:
- `Writer<T>`
- `Reader<T>`
- `Commit<T>`

| Feature Flag | Purpose |
|--------------|---------|
| `serde`      | Enables [`serde`](https://docs.rs/serde)'s `Serialize` & `Deserialize`
| `bincode`    | Enables [`bincode 2.0.0-rc.3`](https://docs.rs/bincode/2.0.0-rc.3/bincode/index.html)'s `Encode` & `Decode`
| `borsh`      | Enables [`borsh`](https://docs.rs/borsh)'s `BorshSerialize` & `BorshDeserialize`

## MSRV
The Minimum Supported Rust Version is `1.70.0`.