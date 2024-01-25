# Writer

| File                 | Purpose |
|----------------------|---------|
| `add_commit_push.rs` | `add()`, `commit()` and any combined functions
| `get.rs`             | Functions related to acquiring new/referenced data
| `misc.rs`            | Miscellaneous functions, e.g, `into_inner()`
| `mod.rs`             | Re-exports only
| `pull.rs`            | `pull()` and any overwriting-like function
| `push.rs`            | `push()` related
| `serde.rs`           | (De)serialization impls
| `tag.rs`             | `tag()` and related
| `timestamp.rs`       | Functions related to timestamps
| `writer.rs`          | `Writer<T>` definition itself, re-usable private functions, and trait impls