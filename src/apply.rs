//---------------------------------------------------------------------------------------------------- Use
use crate::{Writer,Reader,Commit};
use std::collections::HashMap;

//---------------------------------------------------------------------------------------------------- Apply
/// Objects that can be used to "apply" patches to other objects
///
/// This is the trait that objects must implement to be
/// "apply"-able to the target data `T` in a [`Writer<T>`](Writer).
///
/// The generic `Patch` is that object.
///
/// In other words, a [`Writer<T>`] mutates its `T` by [`Apply`]'ing the `Patch` onto `T`.
///
/// You must implement what exactly your `Patch` does to your `T` in the method, [`Apply::sync()`].
///
/// If you have a cheap way to de-duplicate your data, consider re-implementing
/// the [`Apply::sync()`] method, as it gives access to the _most recent_ data.
///
/// ## Return Values
/// [`Apply::apply()`] drops objects within scope and returns nothing.
///
/// This is usually fine as old data is usually discarded
/// anyway in the data structures that match `someday`'s use-case.
///
/// However, if you want access to returned values (e.g, to get back a heavy expensive value)
/// then consider implementing the super-trait [`ApplyReturn`]. It is the exact same as [`Apply`]
/// except that its main [`ApplyReturn::apply_return()`] function allows for returning a value.
///
/// All types that implement [`ApplyReturn`] will automatically
/// implement [`Apply`] and just discard the return value.
///
/// ## Safety
/// **[`Apply::apply()`] must be deterministic.**
///
/// The [`Writer`] may re-apply the same `Patch`'s again onto old
/// cheaply reclaimed data if it has the opportunity to in [`Writer::push()`].
///
/// If [`Apply::apply()`] is not implemented in a deterministic way, it will
/// cause the [`Writer`] and [`Reader`]'s data to drift part.
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
	/// Adding `Patch`'s with [`Writer::add()`] will not call this function.
	///
	/// Note that `patch` is `&mut` to be more flexible, although you must
	/// not mutate `patch` in a way where the next time this is called it will
	/// result in a non-deterministic changes.
	fn apply(patch: &mut Patch, writer: &mut Self, reader: &Self);

	/// Synchronize old data that was reclaimed by the [`Writer`] with the latest data
	///
	/// [`Apply::apply()`] will be called by the [`Writer`] when:
	/// - Using commit operations such as [`Writer::commit()`], [`Writer::overwrite()`], etc.
	///
	/// [`Apply::sync()`] will be called by the [`Writer`] when:
	/// - Re-applying patches to old re-claimed data in [`Writer::push()`]
	///
	/// Note that in case #1, `writer` data will be the most recent local
	/// data and `reader` you'll have access to is old but in case #2,
	/// `old_data` data will be the _old_ reclaimed [`Reader`] data (which is
	/// now the [`Writer`]'s), and `latest_data` will be the _just then_ [`Writer::push()`]'ed data
	/// (which is now viewable by all [`Reader`]'s)
	///
	/// For example:
	///
	/// | Apply Function | `writer` Timestamp | `reader` Timestamp |
	/// |----------------|--------------------|--------------------|
	/// | `apply()`      | 1000               | 999
	/// | `sync()`       | 999                | 1000
	///
	/// In case #2 the [`Writer`] is re-applying your `Patch`'s to the old data.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit local changes.
	/// w.add(PatchString::PushStr("hello".into()));
	///
	/// // Calls `Apply::apply()`
	/// w.commit();
	///
	/// // Calls `Apply::sync()`
	/// // (only if the Writer successfully reclaimed the old data)
	/// w.push();
	/// ```
	/// You can take advantage of this fact in your [`Apply::sync()`] implementation
	/// because you have a direct reference to the latest data in `latest`
	/// that you could use to de-duplicate or otherwise cheaply re-sync data.
	///
	/// By default [`Apply::sync()`] simply calls [`Apply::apply()`]
	/// with your old `Patch`'s onto your old data, however if there
	/// is a cheaper way to "fix" your data, consider re-implementing this method.
	///
	/// The `old_patches` is a [`std::vec::Drain`] iterator that yields
	/// full ownership of the `Patch` when iterated over.
	///
	/// ## Safety
	/// **[`Apply::sync()`] must actually sync `old_data` and `latest_data` to be the same.**
	///
	/// If not, it will cause the [`Writer`] and [`Reader`]'s data to drift part.
	fn sync(old_patches: std::vec::Drain<'_, Patch>, old_data: &mut Self, latest_data: &Self) {
		for mut patch in old_patches {
			Self::apply(&mut patch, old_data, latest_data);
		}
	}
}

//----------------------------------------------------------------------------------------------------
/// Objects that can be used to "apply" patches to other objects and return values
///
/// This trait is as optional extension to the [`Apply`] trait
/// as it functions the same way, although allows for inputting/outputting an
/// arbitrarily infinite amount of generic types for your [`Writer`]'s inner data `T`.
///
/// This lets you have generic patches and return types without:
/// - Boxing a `dyn Trait`
/// - Using `enum` + `match` even when you know _at compile time_ which variant it is
///
/// For example, given this `Patch`:
/// ```rust
/// enum PatchString {
/// 	PushStr(String),
/// 	Take,
/// 	Length,
/// }
/// ```
/// A function cannot abstract over all the
/// different return values that these variants map to:
/// ```rust,ignore
/// // I know at compile time that this is
/// // a `Take` operation, and that it
/// // 100% will return a `String`...
/// let _: CouldBeAnything = map.add(PatchString::Take);
///
/// fn take(patch: PatchTake, s: &mut String) -> CouldBeAnything {
/// 	// But, I still have to match...!
/// 	match patch {
/// 		PatchString::PushStr(s) => s.push_str(s),     // This returns `()`
/// 		PatchString::Take       => std::mem::take(s), // This returns `String`
/// 		PatchString::Length     => s.len(),           // This returns `usize`
/// 	}
/// }
/// ```
/// If we wrapped every return value in a "return enum" so
/// that the above code would compile, it would look like this:
/// ```rust,ignore
/// enum CouldBeAnything {
/// 	Unit(()),
/// 	String(String),
/// 	Usize(usize),
/// }
///
/// let output: CouldBeAnything = writer.add(PatchString::Take);
///
/// // I know at compile type that this is a
/// // `String`, but I still have to `match`...!
/// let actual_value = match output {
/// 	/* useless matching */
/// };
///
/// // Or unnecessarily call unwrap.
/// let actual_value = output.unwrap();
/// ```
/// The usual solution to this is using [dynamic dispatch](https://doc.rust-lang.org/beta/book/ch17-02-trait-objects.html).
///
/// Although, [`ApplyReturn`] combined with [`Writer::commit_return()`] and/or [`Writer::commit_return_iter()`]
/// allow for infinite compile-time functions with generic input that output generic data.
///
/// ## Implementing [`ApplyReturn`]
/// The things `ApplyReturn` needs from you at compile-time to make this work:
/// 1. A generic type `Input` that acts as your input data
/// 2. A generic type `Output` that is the return value from your function
/// 3. Your `Input` must implement [`Into`] for your `Patch` and your
/// [`Apply::apply()`] must behave the same when encountering that `Patch`
///
/// Essentially, if your `Patch` is an `enum`:
/// 1. You create a `struct` mapping to each `enum` variant you want to specialize for
/// 2. You implement [`ApplyReturn`] on that `struct`
/// 3. You pass the `struct` instead of the `enum` variant into [`Writer::commit_return()`] when you want a return value
///
/// Using the above example as a base:
/// ```rust
/// # use someday::{Writer,Apply,ApplyReturn};
/// enum PatchString {
/// 	PushStr(String),
/// 	Take,
/// 	Length,
/// }
///
/// // A specialized struct, specifically for `PatchString::Take`.
/// // This would contain your input data, although
/// // in this case, it is just an empty marker.
/// struct Take;
///
/// // We have to make sure the above type can
/// // 100% losslessly convert to our real `Patch`.
/// impl From<Take> for PatchString {
/// 	fn from(value: Take) -> Self {
/// 		PatchString::Take
/// 	}
/// }
///
/// // Now, we implement `ApplyReturn`, specifying:
/// // - The target data   --------------------------
/// // - The return value  --------------           |
/// // - Our input data    --------     |           |
/// // - Our real `Patch`  --     |     |           |
/// //                      |     |     |           |
/// //                      v     v     v           v
/// impl ApplyReturn<PatchString, Take, String> for String {
/// 	fn apply_return(
/// 		input: &mut Take,
/// 		writer: &mut Self,
/// 		reader: &Self,
/// 	) -> String {
/// 		// Now we just implement the operation as normal,
/// 		// which 100% returns the `String` that we want.
/// 		std::mem::take(writer)
/// 	}
/// }
///
/// // Now, instead of using [`Writer::commit()`] which always returns `()`,
/// // we can use [`Writer::commit_return()`] and get back the value we have specified:
///
/// let (_, mut writer) = someday::new(String::new());
///
/// // Add a regular patch.
/// let patch = PatchString::PushStr("expensive_string_we_want_back".into());
/// # let patch = someday::PatchString::PushStr("expensive_string_we_want_back".into());
/// writer.add(patch).commit();
/// assert_eq!(writer.data(), "expensive_string_we_want_back");
///
/// // `.commit_return()` doesn't take in a regular a
/// // regular `PatchString`, we need to specify our
/// // "special" `ApplyReturn` struct ------------|
/// //                                            |
/// let take = Take;                           // v
/// # let take = someday::PatchStringTake;
/// let string: String = writer.commit_return(take);
///
/// // We got the `String` back.
/// assert_eq!(string, "expensive_string_we_want_back");
/// assert_eq!(writer.data(), "");
/// ```
///
/// # Implementing [`Apply`] "for free"
/// Since [`ApplyReturn`] is a super-set of [`Apply`], you can implement
/// [`Apply`] easily by re-using [`ApplyReturn::apply_return()`] and just
/// dropping the return value:
/// ```rust
/// # use someday::{Apply,ApplyReturn};
/// # enum PatchString { OtherPatches, Take }
/// # struct Take;
/// # impl From<Take> for PatchString {
/// # 	fn from(value: Take) -> Self { PatchString::Take }
/// # }
/// # impl ApplyReturn<PatchString, Take, ()> for String {
/// # 	fn apply_return(input: &mut Take, writer: &mut Self, reader: &Self) {}
/// # }
/// impl Apply<PatchString> for String {
/// 	fn apply(
/// 		patch: &mut PatchString,
/// 		writer: &mut Self,
/// 		reader: &Self,
/// 	) {
/// 		match patch {
/// 			// The normal patches that return `()`.
/// 			PatchString::OtherPatches => (),
///
/// 			/* more matches */
///
/// 			// This one is already implemented by our
/// 			// specialized `Take` struct that implements `ApplyReturn`,
/// 			// so just call that function and drop the value.
/// 			PatchString::Take => { ApplyReturn::apply_return(&mut Take, writer, reader); },
/// 		}
/// 	}
/// }
/// ```
///
/// ## Safety
/// **[`ApplyReturn::apply_return()`] must be deterministic and leave the data in the same
/// state as [`Apply::apply()`] would.**
///
/// When you feed your `Input` into [`Writer::commit_return()`], the `Writer`:
/// 1. Executes your `apply_return()`
/// 2. Returns the value
/// 3. Converts your `Input` into a `Patch` using your implementation of `.into()`
///
/// If the time comes where [`Writer`] has to re-apply your `Patch` during [`Writer::push()`],
/// it will use [`Apply::apply`] and discard your value, even if it has a return value.
///
/// You should be aware of this when implementing [`ApplyReturn`].
pub trait ApplyReturn<Patch, Input, Output>
where
	Self: Clone,
	Patch: From<Input>,
{
	/// Using `input`, apply some changes to the [`Writer`]'s side of data in `writer`.
	///
	/// `reader` provides the most up-to-date copy of the data from the [`Reader`]'s
	/// side, aka, it is the latest [`Commit`] that the `Writer` has [`Writer::push`]'ed.
	///
	/// The reason why you cannot [`Writer::add()`] patches then commit them later is that
	/// [`Writer`] has no way of knowing which `Patch`'s expect return values or not.
	///
	/// Thus, you must add 1 patch and commit it immediately to get the return value back
	/// (or use [`Writer::commit_return_iter()`] to get a iterator of the same value
	/// back).
	fn apply_return(input: &mut Input, writer: &mut Self, reader: &Self) -> Output;
}

/// Objects that can be used to "apply" patches to other objects and return values with lifetimes
///
/// This trait is an optional extension onto [`ApplyReturn`] which itself
/// is an optional extension onto [`Apply`], see [`ApplyReturn`] for more details.
///
/// This documentation will assume you understand [`ApplyReturn`] and why it is needed.
///
/// This trait is solely for usage in [`Writer::commit_return_lt()`].
///
/// ## Returned Lifetime
/// This trait, just like [`ApplyReturn`] returns a generic `Output`
/// that you specify, although it forces your `Output`'s lifetime to
/// be connected with your data.
///
/// This is useful for functions which returned data which has the common pattern of:
/// ```rust
/// # struct Return<'a, T>(std::marker::PhantomData<&'a T>);
/// # trait Asdf<T> {
/// fn function<'a>(&'a mut self) -> Return<'a, T>;
/// # }
/// ```
/// Where the returned `Return` object can only live as long as the `self`.
///
/// A real-world example of this is [`std::vec::Drain`]:
/// ```rust
/// # use someday::*;
/// # use std::vec::Drain;
/// let (r, mut w) = someday::new(vec![0, 1, 2]);
///
/// // Using the regular `commit_return()`
/// // we would get a compile error!
/// //
/// // let entry = w.commit_return(PatchVecDrain);
/// //     ^
/// //     |_ this may live longer than the writer,
/// //        this cannot compile.
///
/// // This is where `commit_return_lt()` is used, which
/// // has lifetime bounds such that your `Writer` _must_
/// // live as least as long as your `Output`
/// let drain: Drain<'_, usize> = w.commit_return_lt(PatchVecDrainAll);
///
/// // We can use `drain` as long as `w` is alive.
/// let numbers: Vec<usize> = drain.collect();
/// assert_eq!(numbers, vec![0, 1, 2]);
///
/// // Attempting to use `entry` again after
/// // this drop would be a compile error.
/// assert!(w.data().is_empty());
/// drop(w);
/// ```
pub trait ApplyReturnLt<'a, Patch, Input, Output>
where
	Self: Clone,
	Patch: From<Input>,
	Output: 'a,
{
	/// Exact same as [`ApplyReturn::apply_return()`], but with lifetime bounds.
	///
	/// [`ApplyReturnLt`] ensures that:
	/// 1. `writer` and `reader` (your data) has a lifetime  of `'a`
	/// 2. Your `Output` also has a lifetime of `'a`
	/// 3. Your `Output` cannot live longer than `writer` and `reader`
	fn apply_return_lt(input: &mut Input, writer: &'a mut Self, reader: &'a Self) -> Output;
}
