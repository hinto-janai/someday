use crate::{Writer,Reader,Commit};
use std::collections::HashMap;

/// Objects that can be used to "apply" patches to other objects
///
/// This is the trait that objects must implement to be
/// "apply"-able to the target data `T` in a [`Writer<T>`](Writer).
///
/// The generic `Patch` is that object.
///
/// In other words, a [`Writer<T>`] mutates its `T` by [`Apply`]'ing the `Patch` onto `T`.
///
/// You must implement what exactly your `Patch` does to your `T` in the method, [`Apply::apply()`].
///
/// ## Safety
/// **[`Apply::apply()`] must be deterministic.**
///
/// The [`Writer`] may re-apply the same `Patch`'s again onto old
/// cheaply reclaimed data if it has the opportunity to in [`Writer::push()`].
///
/// If [`Apply::apply()`] is not implemented in a deterministic way, it will
/// cause the [`Writer`] and [`Reader`]'s data to slowly drift part.
///
/// ## Example
/// In this example, we want a `Writer/Reader` combo guarding a [`HashMap`].
///
/// The "patches" that can be applied to that [`HashMap`] will be our object: `PatchHashMap`.
///
/// ```rust
/// # use someday::*;
/// # use std::cmp::Eq;
/// # use std::hash::Hash;
/// # use std::collections::HashMap;
/// // Our (simple) patches onto a HashMap.
/// enum PatchHashMap<K, V> {
/// 	// Insert the value `V` with the key `K`.
/// 	Insert(K, V),
///
/// 	// Remove the key `K`
/// 	Remove(K),
///
/// 	// Clear the entire HashMap
/// 	Clear,
///
/// 	// Add all Key/Value pairs that exist
/// 	// in the `Reader` but don't in the `Writer`
/// 	AddFromReader
/// }
///
///	// Now, we implement the "Apply" trait on
/// // our `Patch` object, specifying that our
/// // target data `T` is a `HashMap`.
/// impl<K, V> Apply<PatchHashMap<K, V>> for HashMap<K, V>
/// where
/// 	// The key must implement these for HashMap to work.
/// 	K: Clone + Eq + PartialEq + Hash,
/// 	// and both the key and value must be `Clone`-able.
/// 	V: Clone,
/// {
/// 	// This function gives us access to 3 things:
/// 	fn apply(
/// 		// 1. The patch that will be applied
/// 		patch: &mut PatchHashMap<K, V>,
/// 		// 2. The `Writer`'s local side of the data
/// 		// aka, this is a `&mut HashMap<K, V>`
/// 		writer: &mut Self,
/// 		// 3. The _current_ `Reader`'s side of the
/// 		// data, this is a `&HashMap<K, V>` although
/// 		// it isn't necessarily up-to-date!
/// 		reader: &Self,
/// 	) {
/// 		// Match on the patch and do the appropriate action.
/// 		match patch {
/// 			// These must be cloned, since we only have &mut access to them
/// 			// and the `Writer` needs them back ----------|----------|
/// 			//                                            v          v
/// 			PatchHashMap::Insert(k, v) => { writer.insert(k.clone(), v.clone()); }, // These return things so
/// 			PatchHashMap::Remove(k)    => { writer.remove(k); },                    // a scope {} is used to
/// 			PatchHashMap::Clear        => writer.clear(),                           // drop the return values.
/// 			PatchHashMap::AddFromReader => {
/// 				// If a key exists in the `Reader`'s
/// 				// HashMap but not the `Writer`'s, add
/// 				// it to the `Writer`'s.
/// 				for (k, v) in reader.iter() {
/// 					writer
/// 						.entry(k.clone())
/// 						.or_insert_with(|| v.clone());
/// 				}
/// 			}
/// 		}
/// 	}
/// }
///
/// // Let's try it out.
/// fn main() {
/// 	// To make things easier to read:
/// 	// Our HashMap.
/// 	type Map = HashMap<usize, String>;
/// 	// Our Patch.
/// 	type Patch = PatchHashMap<usize, String>;
///
///		// Create a Reader/Writer guarding a `HashMap<usize, String>`,
/// 	// that can have the patch object `PatchHashMap` applied to it.
///		let (r, mut w) = someday::new::<Map, Patch>(Default::default());
///
/// 	// Add a patch.
/// 	// This isn't applied, but rather stored
/// 	// so that it _can_ be applied later.
/// 	w.add(Patch::Insert(0, "hello".into()));
///
///		// Both Reader and Writer still see no changes.
/// 	assert_eq!(w.timestamp(), 0);
/// 	assert_eq!(r.timestamp(), 0);
/// 	assert_eq!(*w.data(), HashMap::new());
/// 	assert_eq!(*r.head(), HashMap::new());
///
/// 	// Add another patch.
/// 	w.add(Patch::Insert(1, "world".into()));
///
/// 	// Commit the data.
/// 	// This will actually call `Apply::apply()`
/// 	// with all the patches saved up so far.
/// 	let commit_info = w.commit();
/// 	assert_eq!(commit_info.patches, 2);
///
/// 	// Now, the Writer can see the changes locally.
/// 	assert_eq!(w.timestamp(), 1);
/// 	assert_eq!(*w.data(), HashMap::from([(0, "hello".into()), (1, "world".into())]));
///
/// 	// The Reader only sees the old copy though.
/// 	assert_eq!(r.timestamp(), 0);
/// 	assert_eq!(r.head(), HashMap::new());
///
/// 	// Push the data (to the Readers).
/// 	let push_info = w.push();
/// 	assert_eq!(push_info.commits, 1);
///
/// 	// Now all Reader's can see the changes.
/// 	assert_eq!(r.timestamp(), 1);
/// 	assert_eq!(r.head(), HashMap::from([(0, "hello".into()), (1, "world".into())]));
///
///		// Writer and Reader are in sync.
/// 	assert_eq!(w.head(), r.head());
/// 	assert_eq!(w.diff(), false);
/// }
/// ```
pub trait Apply<Patch>
where
	Self: Clone,
{
	/// Apply the patch `Patch` to the [`Writer`]'s side of data in `writer`.
	///
	/// `reader` provides the most up-to-date copy of the data from the [`Reader`]'s
	/// side, aka, it is the latest [`Commit`] that the `Writer` has [`Writer::push`]'ed.
	///
	/// This function will be called by the [`Writer`] when:
	/// - Using commit operations such as [`Writer::commit()`], [`Writer::overwrite()`], etc.
	/// - Re-applying patches to old re-claimed data in [`Writer::push()`]
	///
	/// Adding `Patch`'s with [`Writer::add()`] will not call this function.
	///
	/// Note that `patch` is `&mut` to be more flexible, although you must
	/// not mutate `patch` in a way where the next time this is called it will
	/// result in a non-deterministic changes.
	fn apply(patch: &mut Patch, writer: &mut Self, reader: &Self);
}
