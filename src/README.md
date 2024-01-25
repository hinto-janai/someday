# `someday` internals
This is an overview of `someday`'s internals.

The code itself has `grep`-able comments with keywords:

| Word        | Meaning |
|-------------|---------|
| `INVARIANT` | This code makes an _assumption_ that must be upheld for correctness
| `SAFETY`    | This `unsafe` code is okay, for `x,y,z` reasons
| `FIXME`     | This code works but isn't ideal
| `HACK`      | This code is a brittle workaround
| `PERF`      | This code is weird for performance reasons
| `TODO`      | This has to be implemented
| `SOMEDAY`   | This should be implemented... someday

---

# Code Structure
The structure of the folders & files located in `src/`.

| File/Folder    | Purpose |
|----------------|---------|
| `commit.rs`    | `Commit` trait and objects
| `free.rs`      | Free functions, e.g `someday::new()`
| `info.rs`      | `*Info` related objects
| `lib.rs`       | Lints, re-exports only
| `patch.rs`     | `Patch<T>` object
| `reader.rs`    | `Reader<T>` object
| `timestamp.rs` | `Timestamp` alias (usize)
| `writer/`      | `Writer<T>` and all the associated methods

`Writer<T>` is split into its own module as it has _many_ associated methods.

Within `writer/` there is the actual struct definition in `writer.rs` and then each other file follows the same pattern:

```rust
impl<T: Clone> Writer<T> {
	/* ... */
}
```
where the file is a grouping of related associated methods, e.g `push.rs` contains all the `Writer::push_*()` related code.
