# Examples
These are some extremely cherry-picked example situations.

Realistically, `someday` will be much slower than un-contended locks.

| Example   | What                                                       | (Unrealistic, cherry-picked) result |
|-----------|------------------------------------------------------------|-------------------------------------|
| `hashmap` | `RwLock<HashMap>` vs `someday`'s `HashMap`                 | `someday` is 2x-30x faster
| `vec`     | `RwLock<Vec>` vs `someday`'s `Vec`                         | `someday` is 5x-20x faster
| `many`    | Assortment of data structures inside `RwLock` vs `someday` | `someday` is 5x-100x faster

These are cherry-picked, not in order to pretend `someday` will perform like this in all situations, but rather, to showcase what type of situations where it can shine over other synchronization primitives.

Even in slower situations, being lock-free, with `someday` you don't have to worry about:
- Deadlocks
- Poison
- Hanging on a lock for a while
- Etc.
