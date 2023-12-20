//---------------------------------------------------------------------------------------------------- Use
use std::{
	sync::{Arc,
		atomic::{
			AtomicBool,
			Ordering,
		},
	},
	time::Duration,
	borrow::Borrow,
	collections::btree_map::{
		BTreeMap,Entry,
	},
	marker::PhantomData,
};

use crate::{
	INIT_VEC_LEN,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	apply::{Apply,ApplyReturn,ApplyReturnLt},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Writer
/// The single [`Writer`] of some data `T`
///
/// [`Writer`] applies your provided functions onto your data `T`
/// and can [`push()`](Writer::push) that data off to [`Reader`]'s atomically.
///
/// ## Usage
/// This example covers the typical usage of a `Writer`:
/// - Creating some [`Reader`]'s
/// - Adding some `Patch`'s
/// - Viewing the staged `Patch`'s, modifying them
/// - Committing those changes
/// - Pushing those changes to the [`Reader`]'s
///
/// ```rust
/// use someday::{
/// 	{Writer,Reader,Commit,CommitOwned,CommitRef},
/// 	patch::PatchString,
/// };
///
/// // Create a Reader/Writer pair that can "apply"
/// // the `PatchString` patch to `String`'s.
/// let (r, w) = someday::new("".into());
///
/// // To clarify the types of these things:
/// // This is the Reader.
/// // It can clone itself infinite amount of
/// // time very cheaply.
/// let r: Reader<String> = r;
/// for _ in 0..10_000 {
/// 	// pretty cheap operation.
/// 	let another_reader = r.clone();
/// }
///
/// // This is the single Writer, it cannot clone itself.
///	let mut w: Writer<String, PatchString> = w;
///
/// // Both Reader and Writer are at timestamp 0 and see no changes.
/// assert_eq!(w.timestamp(), 0);
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head(), "");
///
/// // The Writer can add many `Patch`'s
/// w.add(PatchString::PushStr("abc".into()));
/// w.add(PatchString::PushStr("def".into()));
/// w.add(PatchString::PushStr("ghi".into()));
/// w.add(PatchString::PushStr("jkl".into()));
///
/// // But `add()`'ing does not actually modify the
/// // local (Writer) or remote (Readers) data, it
/// // just "stages" those for a `commit()`.
/// assert_eq!(w.timestamp(), 0);
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head(), "");
///
/// // We can see our "staged" patches here.
/// let staged: &mut Vec<PatchString> = w.staged();
/// assert_eq!(staged.len(), 4);
/// assert_eq!(staged[0], PatchString::PushStr("abc".into()));
/// assert_eq!(staged[1], PatchString::PushStr("def".into()));
/// assert_eq!(staged[2], PatchString::PushStr("ghi".into()));
/// assert_eq!(staged[3], PatchString::PushStr("jkl".into()));
///
/// // Let's actually remove a patch.
/// let removed = staged.remove(3);
/// assert_eq!(removed, PatchString::PushStr("jkl".into()));
///
/// // Okay, now let's commit locally.
/// let commit_info = w.commit();
/// // We applied 3 patches in total.
/// assert_eq!(commit_info.patches, 3);
/// // And added 1 commit (timestamp).
/// assert_eq!(w.timestamp(), 1);
///
/// // We haven't pushed yet, so the Readers
/// // are still un-aware of our local changes.
/// assert_eq!(w.timestamp(), 1);
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head(), "");
///
/// // Now we push.
/// let push_info = w.push();
/// // We pushed 1 commit in total.
/// assert_eq!(push_info.commits, 1);
/// // Our staged patches are now gone.
/// assert_eq!(w.staged().len(), 0);
///
/// // The Readers are now in sync.
/// assert_eq!(w.timestamp(), 1);
/// assert_eq!(r.timestamp(), 1);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head(), "abcdefghi");
/// ```
pub struct Writer<T>
where
	T: Clone,
{
	// The writer's local mutually
	// exclusive copy of the data.
	//
	// This is an `Option` only because there's
	// a brief moment in `push()` where we need
	// to send off `local`, but we can't yet swap it
	// with the old data.
	//
	// It will be `None` in-between those moments and
	// the invariant is that is MUST be `Some` before
	// `push()` is over.
	//
	// `.unwrap_unchecked()` will be used which will panic in debug builds.
	//
	// MaybeUninit probably works too but clippy is sending me spooky lints.
	pub(super) local: Option<CommitOwned<T>>,

	// The current data the remote `Reader`'s can see.
	pub(super) remote: Arc<CommitOwned<T>>,

	// The AtomicPtr that `Reader`'s enter through.
	// Calling `.load()` would load the `remote` above.
	pub(super) arc: Arc<arc_swap::ArcSwap<CommitOwned<T>>>,

	// Functions that have not yet been applied.
	pub(super) functions: Vec<Box<dyn FnMut(&mut T, &T) + 'static>>,

	// Functions that were already applied,
	// that must be re-applied to the old `T`.
	pub(super) functions_old: Vec<Box<dyn FnMut(&mut T, &T) + 'static>>,

	// This signifies to the `Reader`'s that the
	// `Writer` is currently attempting to swap data.
	//
	// `Reader`'s can cooperate by sleeping
	// for a bit when they see this as `true`
	pub(super) swapping: Arc<AtomicBool>,

	// Tags.
	pub(super) tags: BTreeMap<Timestamp, CommitRef<T>>,
}

//---------------------------------------------------------------------------------------------------- Writer
impl<T> Writer<T>
where
	T: Clone,
{
	#[inline]
	/// Cheaply construct a [`Reader`] connected to this [`Writer`]
	///
	/// This creates a new [`Reader`] that can read all the
	/// data [`push()`](Writer::push)'ed from this [`Writer`].
	///
	/// There is no limit on concurrent [`Reader`]'s.
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // Create 100 more readers.
	/// let readers: Vec<Reader<usize>> = vec![w.reader(); 100];
	/// ```
	pub fn reader(&self) -> Reader<T> {
		Reader {
			arc: Arc::clone(&self.arc),
			swapping: Arc::clone(&self.swapping)
		}
	}

	#[inline]
	/// View the [`Writer`]'s _local_ data
	///
	/// This is the `Writer`'s local data that may or may
	/// not have been [`push()`](Writer::push)'ed yet.
	///
	/// [`commit()`](Writer::commit)'ing will affect this data.
	///
	/// If [`push()`](Writer::push) is called, this would be the
	/// new data that [`Reader`]'s would see.
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // No changes yet.
	/// assert_eq!(*w.data(), 0);
	/// assert_eq!(r.head(),  0);
	///
	/// // Writer commits some changes.
	/// w.add(PatchUsize::Add(1)).commit();
	///
	/// //  Writer sees local change.
	/// assert_eq!(*w.data(), 1);
	/// // Reader doesn't see change.
	/// assert_eq!(r.head(), 0);
	/// ```
	pub fn data(&self) -> &T {
		&self.local_ref().data
	}

	#[inline]
	/// View the latest copy of data [`Reader`]'s have access to
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (_, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // Writer commits some changes.
	/// w.add(PatchUsize::Add(1)).commit();
	///
	/// // Writer sees local change.
	/// assert_eq!(*w.data(), 1);
	/// // But they haven't been pushed to the remote side
	/// // (Readers can't see them)
	/// assert_eq!(*w.data_remote(), 0);
	/// ```
	pub fn data_remote(&self) -> &T {
		&self.remote.data
	}

	#[inline]
	/// View the [`Writer`]'s local "head" [`Commit`]
	///
	/// This is the latest, and local [`Commit`] from the [`Writer`].
	///
	/// Calling [`commit()`](Writer::commit) would make that new
	/// [`Commit`] be the return value for this function.
	///
	/// `Reader`'s may or may not see this [`Commit`] yet.
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (_, mut w) = someday::new::<usize, PatchUsize>(500);
	///
	/// // No changes yet.
	/// let commit: &CommitOwned<usize> = w.head();
	/// assert_eq!(commit.timestamp, 0);
	/// assert_eq!(commit.data,      500);
	///
	/// // Writer commits some changes.
	/// w.add(PatchUsize::Add(1)).commit();
	///
	/// // Head commit is now changed.
	/// let commit: &CommitOwned<usize> = w.head();
	/// assert_eq!(commit.timestamp, 1);
	/// assert_eq!(commit.data,      501);
	/// ```
	pub fn head(&self) -> &CommitOwned<T> {
		self.local_ref()
	}

	#[inline]
	/// View the [`Reader`]'s latest "head" [`Commit`]
	///
	/// This is the latest [`Commit`] the [`Reader`]'s can see.
	///
	/// Calling [`push()`](Writer::push) would update the [`Reader`]'s head [`Commit`].
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (_, mut w) = someday::new::<usize, PatchUsize>(500);
	///
	/// // No changes yet.
	/// let commit: &CommitOwned<usize> = w.head_remote();
	/// assert_eq!(commit.timestamp(), 0);
	/// assert_eq!(*commit.data(),     500);
	///
	/// // Writer commits & pushes some changes.
	/// w.add(PatchUsize::Add(1)).commit_and().push();
	///
	/// // Reader's head commit is now changed.
	/// let commit: &CommitOwned<usize> = w.head_remote();
	/// assert_eq!(commit.timestamp(), 1);
	/// assert_eq!(*commit.data(),     501);
	/// ```
	pub fn head_remote(&self) -> &CommitOwned<T> {
		&*self.remote
	}

	#[inline]
	/// Cheaply acquire ownership of the [`Reader`]'s latest "head" [`Commit`]
	///
	/// This is the latest [`Commit`] the [`Reader`]'s can see.
	///
	/// Calling [`push()`](Writer::push) would update the [`Reader`]'s head [`Commit`].
	///
	/// This is an shared "owned" [`Commit`] (it uses [`Arc`] internally).
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // Reader gets a reference.
	/// let reader: CommitRef<usize> = r.head();
	/// // Writer gets a reference.
	/// let writer: CommitRef<usize> = w.head_remote_ref();
	///
	/// // Reader drops their reference.
	/// // Nothing happens, an atomic count is decremented.
	/// drop(reader);
	///
	/// // Writer drops their reference.
	/// // They were the last reference, so they are
	/// // responsible for deallocating the backing data.
	/// drop(writer);
	/// ```
	pub fn head_remote_ref(&self) -> CommitRef<T> {
		CommitRef { inner: Arc::clone(&self.remote) }
	}

	#[inline]
	/// Add a `Patch` to apply to the data `T`
	///
	/// This does not execute the `Patch` immediately,
	/// it will only store it for later usage.
	///
	/// [`Commit`]-like operations are when these patches
	/// are [`Apply`]'ed to your data.
	///
	/// This returns `self` for method chaining.
	///
	/// ```
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // Add a patch.
	/// w.add(PatchUsize::Add(1));
	///
	/// // It hasn't been applied yet.
	/// assert_eq!(w.staged().len(), 1);
	///
	/// // Now it has.
	/// w.commit();
	/// assert_eq!(w.staged().len(), 0);
	/// ```
	pub fn add<F>(&mut self, f: F) -> &mut Self
	where
		F: FnMut(&mut T, &T) + 'static
	{
		self.functions.push(Box::new(f));
		self
	}

	// #[inline]
	// /// Add multiple `Patch`'s to apply to the data `T`
	// ///
	// /// This is the same as the [`Writer::add`] function but
	// /// it takes an [`Iterator`] of `Patch`'s and add those.
	// ///
	// /// This [`Iterator`] could be [`Vec`], [`slice`], etc.
	// ///
	// /// This returns `self` for method chaining.
	// ///
	// /// ```
	// /// # use someday::*;
	// /// # use someday::patch::*;
	// /// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	// ///
	// /// // Create some Patches.
	// /// let patches: Vec<PatchUsize> =
	// /// 	(0..100)
	// /// 	.map(|u| PatchUsize::Add(u))
	// /// 	.collect();
	// ///
	// /// // Add them.
	// /// w.add_iter(patches.into_iter());
	// ///
	// /// // They haven't been applied yet.
	// /// assert_eq!(w.staged().len(), 100);
	// ///
	// /// // Now they have.
	// /// w.commit();
	// /// assert_eq!(w.staged().len(), 0);
	// /// ```
	// pub fn add_iter<F>(&mut self, functions: impl Iterator<Item = F>) -> &mut Self
	// where
	// 	F: FnMut(&mut T, &T)
	// {
	// 	self.functions.extend(functions);
	// 	self
	// }

	#[inline]
	/// [`Apply`] all the `Patch`'s that were [`add()`](Writer::add)'ed
	///
	/// This will increment the [`Writer`]'s local [`Timestamp`] by `1`,
	/// but only if there were `Patch`'s to actually [`Apply`]. In other
	/// words, if you did not call [`add()`](Writer::add) before this,
	/// [`commit()`](Writer::commit) will do nothing.
	///
	/// This immediately calls [`Apply::apply`] with
	/// your `Patch`'s onto your data `T`.
	///
	/// The new [`Commit`] created from this will become
	/// the [`Writer`]'s new [`Writer::head()`].
	///
	/// You can [`commit()`](Writer::commit) multiple times and
	/// it will only affect the [`Writer`]'s local data.
	///
	/// You can choose when to publish those changes to
	/// the [`Reader`]'s with [`push()`](Writer::push()).
	///
	/// The [`CommitInfo`] object returned is just a container
	/// for some metadata about the [`commit()`](Writer::commit) operation.
	///
	/// ```rust
	/// # use someday::*;
	/// # use someday::patch::*;
	/// let (r, mut w) = someday::new::<usize, PatchUsize>(0);
	///
	/// // Timestamp is 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // And and commit a patch.
	/// w.add(PatchUsize::Add(123)).commit();
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(*w.head(), 123);
	/// ```
	pub fn commit(&mut self) -> CommitInfo {
		self.commit_inner()
	}

	#[inline]
	/// This function is the same as [`Writer::commit()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn commit_and(&mut self) -> &mut Self {
		self.commit_inner();
		self
	}

	// /// Immediately [`commit()`](Writer::commit) an `Input`, and return a value with a lifetime
	// ///
	// /// If your `T` implements [`ApplyReturnLt`] you can specialize
	// /// that some of your `Patch`'s can return values with lifetimes back.
	// ///
	// /// This is optional, but is useful for things like [`std::collections::HashMap::entry()`]
	// /// which returns an object bounded to the lifetime of the [`Writer`]'s data.
	// ///
	// /// This function does not touch any of your current [`staged()`](Writer::staged) `Patch`'s as it:
	// /// - Immediately executes your `input`'s [`ApplyReturn::apply_return()`] implementation
	// /// - Returns you the value
	// ///
	// /// This will increment the [`Writer`]'s local [`Timestamp`] by `1`.
	// ///
	// /// See [`ApplyReturnLt`] for more details.
	// pub fn commit_return_lt<'a, Input, Output>(&'a mut self, mut input: Input) -> Output
	// where
	// 	T: crate::ApplyReturnLt<'a, Patch, Input, Output>,
	// 	Patch: From<Input>,
	// 	Output: 'a,
	// {
	// 	self.local().timestamp += 1;

	// 	// Apply the patch and add to the old vector.
	// 	let return_value = crate::ApplyReturnLt::apply_return_lt(
	// 		&mut input,
	// 		&mut Self::local_field(&mut self.local).data,
	// 		&self.remote.data,
	// 	);

	// 	self.functions_old.push(input.into());

	// 	return_value
	// }

	fn commit_inner(&mut self) -> CommitInfo {
		let patches = self.functions.len();

		// Early return if there was nothing to do.
		if patches == 0 {
			return CommitInfo {
				patches: 0,
				timestamp_diff: self.timestamp_diff(),
			};
		} else {
			self.local().timestamp += 1;
		}

		// Pre-allocate some space for the new patches.
		self.functions_old.reserve_exact(patches);

		// Apply the patches and add to the old vector.
		// for mut patch in self.functions.drain(..) {
		for patch in self.functions.drain(..) {
			// FIXME
			// Apply::apply(
			// 	&mut patch,
			// 	&mut Self::local_field(&mut self.local).data,
			// 	&self.remote.data,
			// );
			self.functions_old.push(patch);
		}

		CommitInfo {
			patches,
			timestamp_diff: self.timestamp_diff(),
		}
	}

	// /// Add multiple `Input`'s  and get a lazily-committing, lazily-returning [`Iterator`]
	// ///
	// /// This is the same as the [`Writer::commit_return`] function but
	// /// it takes an [`Iterator`] of `Input`'s and commits them.
	// ///
	// /// Note that [`Iterator`]'s are _lazy_, so if `.next()` is not called on the returned [`Iterator`]:
	// /// - The `Input`'s won't be applied
	// /// - No [`Commit`]'s will occur
	// /// - No return values will be returned
	// ///
	// /// If either `.next()` is not called OR the input iterator
	// /// contained no patches, the [`Writer`]'s timestamp will _not_
	// /// change.
	// ///
	// /// If there's at least 1 input, the timestamp will increment by 1.
	// ///
	// /// You could selectively look within the returned data, `collect()`,
	// /// them, or do anything you would do with an [`Iterator`].
	// /// ```rust
	// /// # use someday::*;
	// /// # use std::collections::HashMap;
	// /// // Create a HashMap with keys going from 0 to 1,000.
	// /// // with their value to the exact same number, but a String.
	// /// let hashmap = (0..1_000)
	// /// 	.map(|i| (i, format!("{i}")))
	// /// 	.collect::<HashMap<usize, String>>();
	// ///
	// /// // Create Writer with that HashMap.
	// /// let (_, mut writer) = someday::new(hashmap);
	// ///
	// /// assert_eq!(writer.data().len(), 1000);
	// ///
	// /// // We're now going to remove those `0..1_000` key/value pairs.
	// /// let iterator = (0..1_000).map(|i| PatchHashMapRemove(i));
	// ///
	// /// // And store the values in here.
	// /// let mut vec: Vec<String> = vec![];
	// ///
	// /// // This `for` loop simply reads as:
	// /// //
	// /// // For each number 0..1_000, use it as a key into
	// /// // our HashMap, remove that value and return it to us.
	// /// //
	// /// // Each time we iterate (call `.next()` implicitly in this `for` loop) we are:
	// /// // 1. Adding our Patch
	// /// // 2. Committing our Patch
	// /// // 3. Getting the return value
	// /// //
	// /// // If we were to `break` half-way through this iteration,
	// /// // we would leave half of the patches un-touched.
	// /// for (i, return_value) in writer.commit_return_iter(iterator).enumerate() {
	// ///		// To be more clear with the types here:
	// /// 	// Returned from `.enumerate()
	// /// 	let i: usize = i;
	// /// 	// The return value from our patch
	// /// 	let return_value: Option<String> = return_value;
	// ///
	// /// 	let string: String = return_value.unwrap();
	// ///
	// /// 	// Assert it is `i` formatted.
	// /// 	assert_eq!(string, format!("{i}"));
	// ///
	// /// 	// Store.
	// /// 	vec.push(string);
	// /// }
	// ///
	// /// assert_eq!(vec.len(), 1_000);
	// /// assert_eq!(writer.data().len(), 0);
	// /// ```
	// pub fn commit_return_iter<Iter, Input, Output>(&mut self, patches: Iter) -> impl Iterator<Item = Output> + '_
	// where
	// 	T: ApplyReturn<Patch, Input, Output>,
	// 	Iter: Iterator<Item = Input> + 'static,
	// 	Patch: From<Input>,
	// 	Output: 'static,
	// 	Input: 'static,
	// {
	// 	struct CommitReturnIter<'a, T, Patch, Input, Output, Iter>
	// 	where
	// 		T: Apply<Patch> + ApplyReturn<Patch, Input, Output>,
	// 		Iter: Iterator<Item = Input>,
	// 		Patch: From<Input>,
	// 	{
	// 		// Our Writer.
	// 		writer: &'a mut Writer<T>,
	// 		// The iterator of patches.
	// 		patches: Iter,
	// 		// Gets set to `true` if the iterator
	// 		// yielded at least 1 value. It allows
	// 		// to know whether to += 1 the timestamp
	// 		// on drop().
	// 		some: bool,
	// 		_return: PhantomData<Output>,
	// 	}

	// 	impl<T, Patch, Input, Output, Iter> Drop for CommitReturnIter<'_, T, Patch, Input, Output, Iter>
	// 	where
	// 		T: Apply<Patch> + ApplyReturn<Patch, Input, Output>,
	// 		Iter: Iterator<Item = Input>,
	// 		Patch: From<Input>,
	// 	{
	// 		fn drop(&mut self) {
	// 			if self.some {
	// 				self.writer.local().timestamp += 1;
	// 			}
	// 		}
	// 	}

	// 	impl<T, Patch, Input, Output, Iter> Iterator for CommitReturnIter<'_, T, Patch, Input, Output, Iter>
	// 	where
	// 		T: Apply<Patch> + ApplyReturn<Patch, Input, Output>,
	// 		Iter: Iterator<Item = Input>,
	// 		Patch: From<Input>,
	// 	{
	// 		type Item = Output;

	// 		fn next(&mut self) -> Option<Self::Item> {
	// 			match self.functions.next() {
	// 				Some(mut patch) => {
	// 					self.some = true;

	// 					let return_value = ApplyReturn::apply_return(
	// 						&mut patch,
	// 						&mut Writer::local_field(&mut self.writer.local).data,
	// 						&self.writer.remote.data,
	// 					);

	// 					self.writer.patches_old.push(patch.into());
	// 					Some(return_value)
	// 				},
	// 				_ => None,
	// 			}
	// 		}
	// 	}

	// 	CommitReturnIter { writer: self, patches, some: false, _return: PhantomData }
	// }

	#[inline]
	/// Unconditionally push [`Writer`]'s local _committed_ data to the [`Reader`]'s.
	///
	/// This will push changes even if there are no new [`Commit`]'s.
	/// This may be expensive as there are other operations in this
	/// function (memory reclaiming, re-applying patches).
	///
	/// This will return how many [`Commit`]'s the [`Writer`]'s pushed
	/// (aka, how times [`Writer::commit()`] or [`Writer::overwrite()`] or
	/// one of the variants were called)
	///
	/// [`Reader`]'s will atomically be able to access the
	/// the new [`Commit`] before this function is over.
	///
	///	The `Patch`'s that were not [`commit()`](Writer::commit)'ed will not be
	/// pushed and will remain in the [`staged()`](Writer::staged) vector of patches.
	///
	/// The [`PushInfo`] object returned is just a container
	/// for some metadata about the [`push()`](Writer::push) operation.
	///
	/// ## Usage
	/// This function should most likely be combined with a
	/// check to see if there are changes to push:
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	/// w.add(PatchString::PushStr("abc".into()));
	///
	/// if w.ahead() {
	/// 	// won't happen, not yet committed
	/// 	unreachable!();
	/// 	// this call would be wasteful
	/// 	w.push();
	/// }
	///
	/// // Now there are commits to push.
	/// w.commit();
	///
	/// if w.ahead() {
	/// 	let commit_info = w.push();
	/// 	// We pushed 1 commit.
	/// 	assert_eq!(commit_info.commits, 1);
	/// } else {
	/// 	// won't happen
	/// 	unreachable!();
	/// }
	/// ```
	pub fn push(&mut self) -> PushInfo {
		self.swapping_true();
		self.push_inner::<false, ()>(None, None::<fn(&Self)>).0
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_and(&mut self) -> &mut Self {
		self.swapping_true();
		self.push_inner::<false, ()>(None, None::<fn(&Self)>);
		self
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it will [`std::thread::sleep()`] for at least `duration`
	/// amount of time to wait to reclaim the old [`Reader`]'s data.
	///
	/// The `usize` returned is how many [`Commit`]'s the [`Writer`]'s pushed
	/// (aka, how times [`Writer::commit()`] or [`Writer::overwrite()`] or
	/// one of the variants were called) and the `bool` returned is
	/// if the old data was successfully reclaimed or not.
	///
	/// If `duration` has passed, the [`Writer`] will expensively
	/// clone the data as normal and continue on.
	///
	/// This is useful if you know your [`Reader`]'s only
	/// hold onto old data for a brief moment.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{sync::*,thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	/// w.add(PatchString::PushStr("abc".into()));
	/// w.commit();
	///
	/// # let barrier  = Arc::new(Barrier::new(2));
	/// # let other_b = barrier.clone();
	/// let commit = r.head();
	/// spawn(move || {
	///     # other_b.wait();
	/// 	// This `Reader` is holding onto the old data.
	/// 	let moved = commit;
	/// 	// But will let go after 1 millisecond.
	/// 	sleep(Duration::from_millis(1));
	/// });
	///
	/// # barrier.wait();
	/// // Wait 250 milliseconds before resorting to cloning data.
	/// let commit_info = w.push_wait(Duration::from_millis(250));
	/// // We pushed 1 commit.
	/// assert_eq!(commit_info.commits, 1);
	/// // And we successfully reclaimed the old data cheaply.
	/// assert_eq!(commit_info.reclaimed, true);
	/// ```
	pub fn push_wait(&mut self, duration: Duration) -> PushInfo {
		self.swapping_true();
		self.push_inner::<false, ()>(Some(duration), None::<fn(&Self)>).0
	}

	/// This function is the same as [`Writer::push_wait()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_wait_and(&mut self, duration: Duration) -> &mut Self {
		self.swapping_true();
		self.push_inner::<false, ()>(Some(duration), None::<fn(&Self)>);
		self
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it will execute the function `F` in the meanwhile before
	/// attempting to reclaim the old [`Reader`] data.
	///
	/// This can be any arbitrary code, although the function
	/// is provided with the same [`Writer`], `&self`.
	///
	/// The generic `R` is the return value of the function, although
	/// leaving it blank and having a non-returning function will
	/// be enough inference that the return value is `()`.
	///
	/// Basically: "run the function `F` while we're waiting"
	///
	/// This is useful to get some work done before waiting
	/// on the [`Reader`]'s to drop old copies of data.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{sync::*,thread::*,time::*,collections::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// # let barrier  = Arc::new(Barrier::new(2));
	/// # let other_b = barrier.clone();
	/// let head = r.head();
	/// spawn(move || {
	///     # other_b.wait();
	/// 	// This `Reader` is holding onto the old data.
	/// 	let moved = head;
	/// 	// But will let go after 100 milliseconds.
	/// 	sleep(Duration::from_millis(100));
	/// });
	///
	/// # barrier.wait();
	/// // Some work to be done.
	/// let mut hashmap = HashMap::<usize, String>::new();
	/// let mut vec     = vec![];
	///
	/// // Commit.
	/// // Now the `Writer` is ahead by 1 commit, while
	/// // the `Reader` is hanging onto the old one.
	/// w.add(PatchString::PushStr("abc".into())).commit();
	///
	///	// Pass in a closure, so that we can do
	/// // arbitrary things in the meanwhile...!
	/// let (push_info, return_value) = w.push_do(|w| {
	/// 	// While we're waiting, let's get some work done.
	/// 	// Add a bunch of data to this HashMap.
	/// 	(0..1_000).for_each(|i| {
	/// 		hashmap.insert(i, format!("{i}"));
	/// 	});
	/// 	// Add some data to the vector.
	/// 	(0..1_000).for_each(|_| {
	///			vec.push(format!("aaaaaaaaaaaaaaaa"));
	/// 	}); // <- `push_do()` returns `()`
	/// 	# sleep(Duration::from_secs(1));
	/// });     // although we could return anything
	///         // and it would be binded to `return_value`
	///
	/// // At this point, the old `Reader`'s have
	/// // probably all dropped their old references
	/// // and we can probably cheaply reclaim our
	/// // old data back.
	///
	///	// And yes, looks like we got it back cheaply:
	/// assert_eq!(push_info.reclaimed, true);
	///
	/// // And we did some work
	/// // while waiting to get it:
	/// assert_eq!(hashmap.len(), 1_000);
	/// assert_eq!(vec.len(), 1_000);
	/// assert_eq!(return_value, ());
	/// ```
	pub fn push_do<F, R>(&mut self, f: F) -> (PushInfo, R)
	where
		F: FnOnce(&Self) -> R
	{
		self.swapping_true();
		let (push_info, r) = self.push_inner::<false, R>(None, Some(f));

		// SAFETY: we _know_ `R` will be a `Some`
		// because we provided a `Some`. `push_inner()`
		// will always return a Some(value).
		(push_info, unsafe { r.unwrap_unchecked() })
	}

	#[inline]
	/// This function is the same as [`Writer::push_do()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_do_and<F, R>(&mut self, f: F) -> &mut Self
	where
		F: FnOnce(&Self) -> R
	{
		self.swapping_true();
		self.push_inner::<false, R>(None, Some(f));
		self
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it will **always** expensively clone the data
	/// and not attempt to reclaim any old data.
	///
	/// This is useful if you think reclaiming old data
	/// and re-applying your commits would take longer
	/// than just cloning the data itself.
	///
	/// Or if you know your [`Reader`]'s will be holding
	/// onto the data for a long time, and reclaiming data
	/// will be unlikely.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	/// w.add(PatchString::PushStr("abc".into()));
	/// w.commit();
	///
	/// let commit = r.head();
	/// spawn(move || {
	/// 	// This `Reader` will hold onto the old data forever.
	/// 	let moved = commit;
	/// 	loop { std::thread::park(); }
	/// });
	///
	/// // Always clone data, don't wait.
	/// let push_status = w.push_clone();
	/// // We pushed 1 commit.
	/// assert_eq!(push_status.commits, 1);
	/// ```
	pub fn push_clone(&mut self) -> PushInfo {
		// If we're always cloning, there's no
		// need to block Reader's for reclamation
		// so don't set `swapping`.
		self.push_inner::<true, ()>(None, None::<fn(&Self)>).0
	}

	#[inline]
	/// This function is the same as [`Writer::push_clone()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_clone_and(&mut self) -> &mut Self {
		// If we're always cloning, there's no
		// need to block Reader's for reclamation
		// so don't set `swapping`.
		self.push_inner::<true, ()>(None, None::<fn(&Self)>);
		self
	}

	fn push_inner<const CLONE: bool, R>(
		&mut self,
		duration: Option<Duration>,
		function: Option<impl FnOnce(&Self) -> R>,
	) -> (PushInfo, Option<R>)
	{
		// SAFETY: we're temporarily "taking" our `self.local`.
		// It will be uninitialized for the time being.
		// We need to initialize it before returning.
		let local = self.local_take();
		// Swap the reader's `arc_swap` with our new local.
		let old = self.arc.swap(Arc::new(local));

		if !CLONE { self.swapping_false(); }

		// To keep the "swapping" phase as small
		// as possible to not block `Reader`'s, these
		// operations are done here.
		//
		// `self.arc` now returns the new data.
		self.remote = self.arc.load_full();
		let timestamp_diff = self.remote.timestamp - old.timestamp;

		// Re-acquire a local copy of data.

		// Return early if the user wants to deep-clone no matter what.
		if CLONE {
			self.local = Some((*self.remote).clone());
			self.functions_old.clear();
			return (PushInfo {
				timestamp: self.remote.timestamp,
				commits: timestamp_diff,
				reclaimed: false,
			}, None)
		}

		// If the user wants to execute a function
		// while waiting, do so and get the return value.
		let return_value = function.map(|f| f(&self));

		// Try to reclaim data.
		let (mut local, reclaimed) = match Arc::try_unwrap(old) {
			// If there are no more dangling readers on the
			// old Arc we can cheaply reclaim the old data.
			Ok(old) => (old, true),

			// Else, if the user wants to
			// sleep and try again, do so.
			Err(old) => {
				if let Some(duration) = duration {
					// Sleep.
					std::thread::sleep(duration);
					// Try again.
					if let Some(old) = Arc::into_inner(old) {
						(old, true)
					} else {
						((*self.remote).clone(), false)
					}
				} else {
					// Else, there are dangling readers left.
					// As to not wait on them, just expensively clone
					// the inner data to have a mutually exclusive
					// up-to-date local copy.
					((*self.remote).clone(), false)
				}
			},
		};

		// INVARIANT: ALL the branches above must
		// set `self.swapping` to `false` or else
		// we're in a lot of trouble and will lock `Reader`'s.

		if reclaimed {
			// Re-apply patches to this old data.
			// FIXME
			// Apply::sync(self.functions_old.drain(..), &mut local.data, &self.remote.data);
			// Set proper timestamp if we're reusing old data.
			local.timestamp = self.remote.timestamp;
		}

		// Re-initialize `self.local`.
		self.local = Some(local);

		// Clear patches.
		self.functions_old.clear();

		// Output how many commits we pushed.
		(PushInfo {
			timestamp: self.remote.timestamp,
			commits: timestamp_diff,
			reclaimed
		}, return_value)
	}

	#[inline]
	/// Unconditionally overwrite the [`Writer`]'s local [`Commit`] with the current [`Reader`] [`Commit`]
	///
	/// The [`Writer`]'s old local [`Commit`] is returned.
	///
	/// All `Patch`'s that have been already [`commit()`](Writer::commit)'ed are discarded ([`Writer::committed_patches()`]).
	///
	/// Staged `Patch`'s that haven't been [`commit()`](Writer::commit) still kept around ([`Writer::staged()`]).
	///
	/// The [`PullInfo`] object returned is just a container
	/// for some metadata about the [`pull()`](Writer::pull) operation.
	///
	/// ## ⚠️ Warning
	/// This overwrites your [`Writer`]'s data!
	///
	/// Like a `git pull --force`!
	///
	/// It will also reset your [`Writer`]'s [`Timestamp`] to whatever your [`Reader`]'s was.
	///
	/// Like [`Writer::push()`], this will not check for any data
	/// or timestamp differences.
	///
	/// Regardless if the [`Reader`] has old or new data, this will
	/// completely overwrite the [`Writer`]'s local data with it.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit local changes.
	/// w.add(PatchString::PushStr("hello".into())).commit();
	/// assert_eq!(w.head(), "hello");
	///
	/// // Reader's sees nothing
	/// assert_eq!(r.head(), "");
	///
	/// // Pull from the Reader.
	/// let pull_status = w.pull();
	/// assert_eq!(pull_status.old_writer_data, "hello");
	///
	///	// We're back to square 1.
	/// assert_eq!(w.head(), "");
	/// ```
	pub fn pull(&mut self) -> PullInfo<T> {
		self.pull_inner()
	}

	#[inline]
	/// This function is the same as [`Writer::pull()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn pull_and(&mut self) -> &mut Self {
		self.pull_inner();
		self
	}

	#[inline]
	fn pull_inner(&mut self) -> PullInfo<T> {
		// Delete old patches, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.functions_old.clear();
		PullInfo {
			commits_reverted: self.timestamp_diff(),
			old_writer_data: self.local_swap((*self.remote).clone()),
		}
	}

	#[inline]
	/// Overwrite the [`Writer`]'s local data with `data`.
	///
	/// The [`Writer`]'s old local data is returned.
	///
	/// All `Patch`'s that have been already [`commit()`](Writer::commit)'ed are discarded ([`Writer::committed_patches()`]).
	///
	/// Staged `Patch`'s that haven't been [`commit()`](Writer::commit) still kept around ([`Writer::staged()`]).
	///
	/// This increments the [`Writer`]'s local [`Timestamp`] by `1`.
	///
	/// A [`Patch`](Apply) that overwrites the data
	/// applied with [`Writer::commit()`] would be
	/// equivalent to this convenience function.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Push changes.
	/// w
	/// 	.add(PatchString::PushStr("hello".into()))
	/// 	.commit_and() // <- commit 1
	/// 	.push();
	///
	/// assert_eq!(w.timestamp(), 1);
	///
	/// // Reader's sees them.
	/// assert_eq!(r.head(), "hello");
	/// assert_eq!(r.timestamp(), 1);
	///
	/// // Commit some changes.
	/// w.add(PatchString::Assign("hello".into())).commit(); // <- commit 2
	/// w.add(PatchString::Assign("hello".into())).commit(); // <- commit 3
	/// w.add(PatchString::Assign("hello".into())).commit(); // <- commit 4
	/// assert_eq!(w.committed_patches().len(), 3);
	///
	/// // Overwrite the Writer with arbitrary data.
	/// let old_data = w.overwrite(String::from("world")); // <- commit 5
	/// assert_eq!(old_data, "hello");
	/// // Committed patches were deleted.
	/// assert_eq!(w.committed_patches().len(), 0);
	///
	///	// Push that change.
	/// w.push();
	///
	/// // Readers see change.
	/// assert_eq!(r.head(), "world");
	///
	/// // 5 commits total.
	/// assert_eq!(w.timestamp(), 5);
	/// assert_eq!(r.timestamp(), 5);
	/// ```
	pub fn overwrite(&mut self, data: T) -> CommitOwned<T> {
		self.overwrite_inner(data)
	}

	#[inline]
	/// This function is the same as [`Writer::overwrite()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn overwrite_and(&mut self, data: T) -> &mut Self {
		self.overwrite_inner(data);
		self
	}

	#[inline(always)]
	// `T` might be heavy to stack copy, so inline this.
	fn overwrite_inner(&mut self, data: T) -> CommitOwned<T> {
		// Delete old patches, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.functions_old.clear();
		self.local_swap(CommitOwned { timestamp: self.timestamp() + 1, data })
	}

	#[inline]
	/// Store the latest [`Reader`] head [`Commit`] (cheaply)
	///
	/// This stores the latest [`Reader`] [`Commit`]
	/// (aka, whatever [`Reader::head()`] would return)
	/// into the [`Writer`]'s local storage.
	///
	/// These tags can be inspected later with [`Writer::tags()`].
	///
	/// If [`Writer::tag()`] is never used, it will never allocate space.
	///
	/// This returns the tagged [`CommitRef`] that was stored.
	///
	/// ## Why does this exist?
	/// You could store your own collection of [`CommitRef`]'s alongside
	/// your [`Writer`] and achieve similar results, however there are
	/// benefits to [`Writer`] coming with one built-in:
	///
	/// 1. It logically associates [`Commit`]'s with a certain [`Writer`]
	/// 2. The invariant that all [`Commit`]'s tagged are/were valid [`Commit`]'s
	/// to both the [`Writer`] and [`Reader`] is always upheld as the [`Writer`]
	/// does not provide mutable access to the inner [`Commit`] data or [`Timestamp`]'s
	///
	/// ## Note
	/// This stores the **Reader's** latest [`Commit`], not the Writer's.
	///
	/// The reason why the [`Writer`]'s commit cannot be tagged is that
	/// the [`Writer`]'s commit is a local, mutable, non-shared `CommitOwned<T>`
	/// instead of a shared `CommitRef<T>`, thus tagging it would require
	/// cloning it into another "shared" copy, which may be expensive.
	///
	/// You can always [`Writer::push_and()`] + [`Writer::tag()`]
	/// to push the latest commit, then tag it.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Push a change.
	/// w.add(PatchString::PushStr("a".into())).commit_and().push();
	///
	///	// Tag that change, and clone it (this is cheap).
	/// let tag = CommitRef::clone(w.tag());
	///
	/// // This tag is the same as the Reader's head Commit.
	/// assert_eq!(tag, r.head());
	/// assert_eq!(tag.timestamp(), 1);
	///
	/// // Push a whole bunch changes.
	/// for _ in 0..100 {
	/// 	w.add(PatchString::PushStr("b".into())).commit_and().push();
	/// }
	///	assert_eq!(w.timestamp(), 101);
	///	assert_eq!(r.timestamp(), 101);
	///
	/// // Writer is still holding onto the tag, so remove it.
	/// let removed_tag = w.tag_remove(tag.timestamp()).unwrap();
	/// assert_eq!(removed_tag, tag);
	///
	/// // Atomically decrements a counter.
	/// drop(removed_tag);
	///
	/// // SAFETY: now we know that we're the
	/// // only ones holding onto this commit.
	/// let inner_data: String = tag.try_unwrap().unwrap().data;
	///
	/// // Now, let's use that old tag to overwrite our current data.
	/// //
	/// // Note that the Writer can _never_ "rebase" and go back in time
	/// // (at least, before the Reader's timestamp).
	/// //
	/// // This overwrite operation is the same as a regular commit,
	/// // it takes the data and adds 1 to the timestamp, it does
	/// // not reset the timestamp.
	/// w.overwrite(inner_data);
	/// assert_eq!(w.timestamp(), 102);
	/// assert_eq!(w.data(), "a");
	/// ```
	pub fn tag(&mut self) -> &CommitRef<T> {
		self.tags.entry(self.remote.timestamp)
			.or_insert_with(|| CommitRef { inner: Arc::clone(&self.remote) })
	}

	/// This function is the same as [`Writer::tag()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn tag_and(&mut self) -> &mut Self {
		if let Entry::Vacant(entry) = self.tags.entry(self.remote.timestamp) {
			let head_remote = Arc::clone(&self.remote);
			entry.insert(CommitRef { inner: head_remote });
		}
		self
	}

	#[inline]
	/// Clear all the stored [`Writer`] tags
	///
	/// This calls [`BTreeMap::clear()`] on this [`Writer`]'s internal tags.
	pub fn tag_clear(&mut self) -> &mut Self {
		self.tags.clear();
		self
	}

	/// Retains only the tags specified by the predicate
	///
	/// In other words, remove all tags for which `F` returns false.
	///
	/// The elements are visited in ascending key order.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut writer) = someday::new::<String, PatchString>("aaa".into());
	///
	/// // Tag this "aaa" commit.
	/// writer.tag();
	///
	/// // Push and tag a whole bunch changes.
	/// for i in 1..100 {
	/// 	writer
	/// 		.add(PatchString::Assign("bbb".into()))
	/// 		.commit_and()
	/// 		.push_and()
	/// 		.tag();
	/// }
	///
	/// assert_eq!(writer.tags().len(), 100);
	///
	/// // Only retain the tags where the
	/// // commit data value is "aaa".
	/// writer.tag_retain(|commit| *commit.data() == "aaa");
	///
	/// // Just 1 tag now.
	/// assert_eq!(writer.tags().len(), 1);
	/// assert_eq!(*writer.tags().get(&0).unwrap().data(), "aaa");
	/// ```
	pub fn tag_retain<F>(&mut self, f: F) -> &mut Self
	where
		F: Fn(&CommitRef<T>) -> bool,
	{
		// The normal `retain()` gives `&mut` access to the
		// 2nd argument, but we need to uphold the invariant
		// that timestamps + commits are always valid, and
		// never randomly mutated by the user.
		//
		// So, we will iterate over our btree, marking
		// which timestamps we need to remove, then sweep
		// them all after.

		self.tags
			.iter_mut()                  // for each key, value
			.filter(|(_, v)| !f(v))      // yield if F returns false
			.map(|(k, _)| *k)            // yield the timestamp
			.collect::<Vec<Timestamp>>() // collect them
			.into_iter()                 // for each timestamp
			.for_each(|timestamp| { self.tags.remove(&timestamp); }); // remove it

		self
	}

	#[inline]
	/// Remove a stored tag from the [`Writer`]
	///
	/// This calls [`BTreeMap::remove()`] on this [`Writer`]'s internal tags.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut writer) = someday::new::<String, PatchString>("aaa".into());
	///
	/// let tag = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_remove(tag.timestamp()).unwrap();
	///
	/// assert_eq!(tag, removed);
	/// ```
	pub fn tag_remove(&mut self, timestamp: Timestamp) -> Option<CommitRef<T>> {
		self.tags.remove(&timestamp)
	}

	#[inline]
	/// This function is the same as [`Writer::tag_remove()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn tag_remove_and(&mut self, timestamp: Timestamp) -> &mut Self {
		self.tags.remove(&timestamp);
		self
	}

	#[inline]
	/// Removes and returns the oldest tag from the [`Writer`]
	///
	/// The [`CommitRef`] returned is the _oldest_ one (smallest [`Timestamp`]).
	///
	/// This calls [`BTreeMap::pop_first()`] on this [`Writer`]'s internal tags.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut writer) = someday::new::<String, PatchString>("aaa".into());
	///
	/// let tag_0 = CommitRef::clone(&writer.tag());
	/// let tag_1 = CommitRef::clone(&writer.tag());
	/// let tag_2 = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_pop_oldest().unwrap();
	///
	/// assert_eq!(tag_0, removed);
	/// ```
	pub fn tag_pop_oldest(&mut self) -> Option<CommitRef<T>> {
		self.tags.pop_first().map(|(_, c)| c)
	}

	#[inline]
	/// Removes and returns the last tag from the [`Writer`]
	///
	/// The [`CommitRef`] returned is the _latest_ one (largest [`Timestamp`]).
	///
	/// This calls [`BTreeMap::pop_last()`] on this [`Writer`]'s internal tags.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut writer) = someday::new::<String, PatchString>("aaa".into());
	///
	/// let tag_0 = CommitRef::clone(&writer.tag());
	/// let tag_1 = CommitRef::clone(&writer.tag());
	/// let tag_2 = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_pop_latest().unwrap();
	///
	/// assert_eq!(tag_2, removed);
	/// ```
	pub fn tag_pop_latest(&mut self) -> Option<CommitRef<T>> {
		self.tags.pop_last().map(|(_, c)| c)
	}

	#[inline]
	/// This function is the same as [`Writer::tag_pop_oldest()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn tag_pop_oldest_and(&mut self) -> &mut Self {
		self.tags.pop_first();
		self
	}

	#[inline]
	/// This function is the same as [`Writer::tag_pop_latest()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn tag_pop_latest_and(&mut self) -> &mut Self {
		self.tags.pop_last();
		self
	}

	#[inline]
	/// If the [`Writer`]'s local [`Commit`] is different than the [`Reader`]'s
	///
	/// Compares the [`Commit`] that the [`Reader`]'s can
	/// currently access with the [`Writer`]'s current local [`Commit`].
	///
	/// This returns `true` if both:
	/// - The data is different
	/// - The [`Timestamp`] is different
	///
	/// Note that this includes non-[`push()`](Writer::push)'ed [`Writer`] data.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit but don't push.
	/// w.add(PatchString::PushStr("abc".into())).commit();
	///
	/// // Writer and Reader's commit is different.
	/// assert!(w.diff());
	/// ```
	pub fn diff(&self) -> bool
	where T:
		PartialEq<T>
	{
		self.local_ref().diff(&*self.remote)
	}

	#[inline]
	/// If the [`Writer`]'s local [`Timestamp`] is greater than the [`Reader`]'s [`Timestamp`]
	///
	/// Compares the timestamp of the [`Reader`]'s currently available
	/// data with the [`Writer`]'s current local timestamp.
	///
	/// This returns `true` if the [`Writer`]'s timestamp
	/// is greater than [`Reader`]'s timestamp (which means
	/// [`Writer`] is ahead of the [`Reader`]'s)
	///
	/// Note that this does not check the data itself, only the [`Timestamp`].
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit 10 times but don't push.
	/// for i in 0..10 {
	/// 	w.add(PatchString::PushStr("abc".into())).commit();
	/// }
	///
	/// // Writer at timestamp 10.
	/// assert_eq!(w.timestamp(), 10);
	///
	/// // Reader at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
	///
	/// // Writer is ahead of the Reader's.
	/// assert!(w.ahead());
	/// ```
	pub fn ahead(&self) -> bool {
		self.local_ref().ahead(&*self.remote)
	}

	#[inline]
	/// If the [`Writer`]'s local [`Timestamp`] is greater than an arbitrary [`Commit`]'s [`Timestamp`]
	///
	/// This takes any type of [`Commit`], so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit 10 times.
	/// for i in 0..10 {
	/// 	w.add(PatchString::PushStr("abc".into())).commit();
	/// }
	/// // At timestamp 10.
	/// assert_eq!(w.timestamp(), 10);
	///
	/// // Create fake `CommitOwned`
	/// let fake_commit = CommitOwned {
	/// 	timestamp: 1,
	/// 	data: String::new(),
	/// };
	///
	/// // Writer is ahead of that commit.
	/// assert!(w.ahead_of(&fake_commit));
	/// ```
	pub fn ahead_of(&self, commit: &impl Commit<T>) -> bool {
		self.local_ref().ahead(commit)
	}

	#[inline]
	/// If the [`Writer`]'s local [`Timestamp`] is less than an arbitrary [`Commit`]'s [`Timestamp`]
	///
	/// This takes any type of [`Commit`], so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // At timestamp 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Create fake `CommitOwned`
	/// let fake_commit = CommitOwned {
	/// 	timestamp: 1000,
	/// 	data: String::new(),
	/// };
	///
	/// // Writer is behind that commit.
	/// assert!(w.behind(&fake_commit));
	/// ```
	pub fn behind(&self, commit: &impl Commit<T>) -> bool {
		self.local_ref().behind(commit)
	}

	#[inline]
	/// Get the current [`Timestamp`] of the [`Writer`]'s local [`Commit`]
	///
	/// This returns the number indicating the [`Writer`]'s data's version.
	///
	/// This number starts at `0`, increments by `1` every time a [`Writer::commit()`]
	/// -like operation is called, and it will never be less than the [`Reader`]'s [`Timestamp`].
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // At timestamp 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Commit some changes.
	/// w.add(PatchString::PushStr("abc".into())).commit();
	///
	/// // At timestamp 1.
	/// assert_eq!(w.timestamp(), 1);
	/// // We haven't pushed, so Reader's
	/// // are still at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
	/// ```
	pub fn timestamp(&self) -> Timestamp {
		self.local_ref().timestamp
	}

	#[inline]
	/// Get the current [`Timestamp`] of the [`Reader`]'s "head" [`Commit`]
	///
	/// This returns the number indicating the [`Reader`]'s data's version.
	///
	/// This will never be greater than the [`Writer`]'s timestamp.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // At timestamp 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Commit some changes.
	/// w.add(PatchString::PushStr("abc".into())).commit();
	///
	/// // Writer is at timestamp 1.
	/// assert_eq!(w.timestamp(), 1);
	/// // We haven't pushed, so Reader's
	/// // are still at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
	///
	/// // Push changes
	/// w.push();
	///
	/// // Readers are now up-to-date.
	/// assert_eq!(r.timestamp(), 1);
	/// ```
	pub fn timestamp_remote(&self) -> Timestamp {
		self.remote.timestamp
	}

	#[inline]
	/// Get the difference between the [`Writer`]'s and [`Reader`]'s [`Timestamp`]
	///
	/// This returns the number indicating how many commits the
	/// [`Writer`] is ahead on compared to the [`Reader`]'s.
	///
	/// In other words, it is: `writer_timestamp - reader_timestamp`
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // At timestamp 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Push 1 change.
	/// w.add(PatchString::PushStr("abc".into())).commit_and().push();
	///
	/// // Commit 5 changes locally.
	/// for i in 0..5 {
	/// 	w.add(PatchString::PushStr("abc".into())).commit();
	/// }
	///
	/// // Writer is at timestamp 5.
	/// assert_eq!(w.timestamp(), 6);
	/// // Reader's are still at timestamp 1.
	/// assert_eq!(r.timestamp(), 1);
	///
	/// // The difference is 5.
	/// assert_eq!(w.timestamp_diff(), 5);
	/// ```
	pub fn timestamp_diff(&self) -> usize {
		self.local_ref().timestamp - self.remote.timestamp
	}

	// /// Restore all the staged changes.
	// ///
	// /// This removes all the `Patch`'s that haven't yet been [`commit()`](Writer::commit)'ed.
	// ///
	// /// Calling `Writer::staged().drain(..)` would be equivalent.
	// ///
	// /// If there are `Patch`'s, this function will remove
	// /// and return them as a [`std::vec::Drain`] iterator.
	// ///
	// /// If there are no `Patch`'s, this will return [`None`].
	// ///
	// /// Dropping the [`std::vec::Drain`] will drop the `Patch`'s.
	// ///
	// /// ```rust
	// /// # use someday::{*,patch::*};
	// /// # use std::{thread::*,time::*};
	// /// let (r, mut w) = someday::new::<String, PatchString>("".into());
	// ///
	// /// // Add some changes, but don't commit.
	// /// w.add(PatchString::PushStr("abc".into()));
	// /// assert_eq!(w.staged().len(), 1);
	// ///
	// ///	// Restore changes.
	// /// let drain = w.restore();
	// /// match drain {
	// /// 	Some(removed) => assert_eq!(removed.count(), 1),
	// /// 	_ => unreachable!(),
	// /// }
	// /// ```
	// pub fn restore(&mut self) -> Option<std::vec::Drain<'_, Patch>> {
	// 	if self.functions.is_empty() {
	// 		None
	// 	} else {
	// 		Some(self.functions.drain(..))
	// 	}
	// }

	// #[inline]
	// /// All the `Patch`'s that **haven't** been [`commit()`](Writer::commit)'ed yet, aka, "staged" changes
	// ///
	// /// You are allowed to do anything to these `Patch`'s as they haven't
	// /// been committed yet and the `Writer` does not necessarily  need them.
	// ///
	// /// You can use something like `.staged().drain(..)` to get back all the `Patch`'s.
	// ///
	// /// All the `Patch`'s that have been [`commit()`](Writer::commit)'ed but not yet
	// /// [`push()`](Writer::push)'ed are safely stored internally by the [`Writer`].
	// ///
	// /// ```rust
	// /// # use someday::{*,patch::*};
	// /// # use std::{thread::*,time::*};
	// /// let (r, mut w) = someday::new::<String, PatchString>("".into());
	// ///
	// /// // Add some changes.
	// /// let change = PatchString::PushStr("abc".into());
	// /// w.add(change.clone());
	// ///
	// /// // We see and mutate the staged changes.
	// /// assert_eq!(w.staged().len(), 1);
	// /// assert_eq!(w.staged()[0], change);
	// ///
	// /// // Let's actually remove that change.
	// /// let removed = w.staged().remove(0);
	// /// assert_eq!(w.staged().len(), 0);
	// /// assert_eq!(change, removed);
	// /// ```
	// pub fn staged(&mut self) -> &mut Vec<Patch> {
	// 	&mut self.functions
	// }

	#[inline]
	/// Output all the tagged [`Commit`]'s
	///
	/// This returns a [`BTreeMap`] where the:
	/// - Key is the [`Commit`]'s [`Timestamp`], and the
	/// - Value is the shared [`CommitRef`] object itself
	///
	/// Mutable access to these tags are restricted in a way
	/// such that these tags are guaranteed to have been valid
	/// [`Commit`]'s that were [`push()`](Writer::push)'ed to the [`Reader`]'s.
	///
	/// Aka, these tags will never be arbitrary data.
	///
	/// Therefore the [`Timestamp`] and [`CommitRef`] data can be relied upon.
	///
	/// These "tags" are created with [`Writer::tag()`].
	pub fn tags(&self) -> &BTreeMap<Timestamp, CommitRef<T>> {
		&self.tags
	}

	// #[inline]
	// /// All the `Patch`'s that **have** been [`commit()`](Writer::commit)'ed but not yet [`push()`](Writer::push)'ed
	// ///
	// /// You are not allowed to mutate these `Patch`'s as they haven't been
	// /// [`push()`](Writer::push)'ed yet and the `Writer` may need them in the future.
	// ///
	// /// ```rust
	// /// # use someday::{*,patch::*};
	// /// # use std::{thread::*,time::*};
	// /// let (r, mut w) = someday::new::<String, PatchString>("".into());
	// ///
	// /// // Commit some changes.
	// /// let change = PatchString::PushStr("abc".into());
	// /// w.add(change.clone());
	// /// w.commit();
	// ///
	// /// // We can see but not mutate patches.
	// /// assert_eq!(w.committed_patches().len(), 1);
	// /// assert_eq!(w.committed_patches()[0], change);
	// /// ```
	// pub fn committed_patches(&self) -> &Vec<Patch> {
	// 	&self.functions_old
	// }

	#[inline]
	/// How many [`Reader`]'s are _currently_ accessing
	/// the current [`Reader`] head [`Commit`]?
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // The Writer, `w` holds 2 strong counts.
	/// assert_eq!(w.head_readers(), 2);
	///
	/// // Create and leak 8 Reader's.
	/// // Note however, the above Reader's
	/// // do not have strong references to the
	/// // underlying data, so they don't count.
	/// for i in 0..8 {
	/// 	let reader = w.reader();
	/// 	std::mem::forget(reader);
	/// }
	/// let r = w.reader();
	/// assert_eq!(w.head_readers(), 2);
	///
	/// // Leak the actual data 8 times.
	/// for i in 0..8 {
	/// 	let head: CommitRef<String> = r.head();
	/// 	std::mem::forget(head);
	/// }
	///
	/// // Now there are 10 strong references.
	/// // (which will never be reclaimed since
	/// // we just leaked them)
	/// assert_eq!(w.head_readers(), 10);
	/// ```
	pub fn head_readers(&self) -> usize {
		Arc::strong_count(&self.remote)
	}

	#[inline]
	/// How many [`Reader`]'s are there?
	///
	/// Unlike [`Writer::head_readers()`], this doesn't count references
	/// to the data, it counts how many [`Reader`] objects are in existence.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // 2 Reader's (the Writer counts as a Reader).
	/// assert_eq!(w.reader_count(), 2);
	///
	/// // Create and leak 8 Reader's.
	/// for i in 0..8 {
	/// 	let reader = r.clone();
	/// 	std::mem::forget(reader);
	/// }
	///
	/// // Now there are 10.
	/// assert_eq!(w.reader_count(), 10);
	/// ```
	pub fn reader_count(&self) -> usize {
		Arc::strong_count(&self.arc)
	}

	// /// Get the current status on the [`Writer`] and [`Reader`]
	// ///
	// /// This is a bag of various metadata about the current
	// /// state of the [`Writer`] and [`Reader`].
	// ///
	// /// If you only need 1 or a few of the fields in [`StatusInfo`],
	// /// consider using their individual methods instead.
	// pub fn status(&self) -> StatusInfo<'_, T, Patch> {
	// 	StatusInfo {
	// 		staged_patches: &self.functions,
	// 		committed_patches: self.committed_patches(),
	// 		head: self.head(),
	// 		head_remote: self.head_remote(),
	// 		head_readers: self.head_readers(),
	// 		reader_count: self.reader_count(),
	// 		timestamp: self.timestamp(),
	// 		timestamp_remote: self.timestamp_remote(),
	// 	}
	// }

	/// Shrinks the capacity of the `Patch` [`Vec`]'s as much as possible
	///
	/// This calls [`Vec::shrink_to_fit()`] on the 2
	/// internal [`Vec`]'s in [`Writer`] holding:
	/// 1. The currently staged `Patch`'s
	/// 2. The already committed `Patch`'s
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::with_capacity::<String, PatchString>("".into(), 16);
	///
	/// // Capacity is 16.
	/// assert_eq!(w.committed_patches().capacity(), 16);
	/// assert_eq!(w.staged().capacity(),            16);
	///
	/// // Commit 32 `Patch`'s
	/// for i in 0..32 {
	/// 	w.add(PatchString::Assign("".into())).commit();
	/// }
	/// // Stage 16 `Patch`'s
	/// for i in 0..16 {
	/// 	w.add(PatchString::Assign("".into()));
	/// }
	///
	/// // Commit capacity is now 32.
	/// assert_eq!(w.committed_patches().capacity(), 32);
	/// // This didn't change, we already had
	/// // enough space to store them.
	/// assert_eq!(w.staged().capacity(), 16);
	///
	/// // Commit, push, shrink.
	/// w.commit_and().push_and().shrink_to_fit();
	///
	/// // They're now empty and taking 0 space.
	/// assert_eq!(w.committed_patches().capacity(), 0);
	/// assert_eq!(w.staged().capacity(), 0);
	/// ```
	pub fn shrink_to_fit(&mut self) {
		self.functions.shrink_to_fit();
		self.functions_old.shrink_to_fit();
	}

	// /// Consume this [`Writer`] and return the inner components
	// ///
	// /// In left-to-right order, this returns:
	// /// 1. The [`Writer`]'s local data
	// /// 2. The latest [`Reader`]'s [`Commit`] (aka, from [`Reader::head()`])
	// /// 3. The "staged" `Patch`'s that haven't been [`commit()`](Writer::commit)'ed (aka, from [`Writer::staged()`])
	// /// 4. The committed `Patch`'s that haven't been [`push()`](Writer::push)'ed (aka, from [`Writer::committed_patches()`])
	// ///
	// /// ```rust
	// /// # use someday::{*,patch::*};
	// /// # use std::{thread::*,time::*};
	// /// let (r, mut w) = someday::new::<String, PatchString>("".into());
	// ///
	// /// // Commit some changes.
	// /// let committed_change = PatchString::PushStr("a".into());
	// /// w.add(committed_change.clone());
	// /// w.commit();
	// ///
	// /// // Add but don't commit
	// /// let staged_change = PatchString::PushStr("b".into());
	// /// w.add(staged_change.clone());
	// ///
	// /// let (
	// /// 	writer_data,
	// /// 	reader_data,
	// /// 	staged_changes,
	// /// 	committed_changes,
	// /// ) = w.into_inner();
	// ///
	// /// assert_eq!(writer_data, "a");
	// /// assert_eq!(reader_data, ""); // We never `push()`'ed, so Readers saw nothing.
	// /// assert_eq!(staged_changes[0], staged_change);
	// /// assert_eq!(committed_changes[0], committed_change);
	// /// ```
	// pub fn into_inner(mut self) -> (CommitOwned<T>, CommitRef<T>, Vec<Patch>, Vec<Patch>) {
	// 	let local = self.local_take();

	// 	let snap = CommitOwned {
	// 		timestamp: local.timestamp,
	// 		data: local.data,
	// 	};

	// 	(snap, CommitRef { inner: self.remote }, self.functions, self.functions_old)
	// }
}

//---------------------------------------------------------------------------------------------------- Private writer functions
impl<T> Writer<T>
where
	T: Clone
{
	// HACK:
	// These `local_*()` functions are a work around.
	// Writer's local data is almost always initialized, but
	// during `push()` there's a brief moment where we send our
	// data off to the readers, but we haven't reclaimed or cloned
	// new data yet, so our local data is empty (which isn't allowed).
	//
	// `MaybeUninit` may work here but keeping our local data
	// as `Option<T>` then just using `.unwrap_unchecked()` is
	// easier than safely upholding the insane amount of
	// invariants uninitialized memory has.
	//
	// `.unwrap_unchecked()` actually `panic!()`'s on `debug_assertions` too.

	#[inline(always)]
	fn local(&mut self) -> &mut CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.as_mut().unwrap_unchecked() }
	}

	// Same as `local()`, but field specific so we
	// can around taking `&mut self` when we need
	// `&` to `self` as well.
	//
	// SAFETY: This function is ONLY for this `self.local` purpose.
	#[inline(always)]
	fn local_field(local: &mut Option<CommitOwned<T>>) -> &mut CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { local.as_mut().unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_take(&mut self) -> CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.take().unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_swap(&mut self, other: CommitOwned<T>) -> CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.replace(other).unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_inner(self) -> CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_ref(&self) -> &CommitOwned<T> {
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.as_ref().unwrap_unchecked() }
	}

	#[inline(always)]
	fn swapping_true(&mut self) {
		self.swapping.store(true, Ordering::Relaxed);
	}

	#[inline(always)]
	fn swapping_false(&mut self) {
		self.swapping.store(false, Ordering::Release);
	}
}

//---------------------------------------------------------------------------------------------------- Writer trait impl
// impl<T> std::fmt::Debug for Writer<T>
// where
// 	T: Clone + std::fmt::Debug,
// {
// 	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// 		f.debug_struct("CommitOwned")
// 			.field("local", &self.local)
// 			.field("remote", &self.remote)
// 			.field("arc", &self.arc)
// 			.field("patches", &self.functions)
// 			.field("patches_old", &self.functions_old)
// 			.field("swapping", &self.swapping)
// 			.field("tags", &self.tags)
// 			.finish()
// 	}
// }

// impl<T> PartialEq for Writer<T>
// where
// 	T: Clone + PartialEq,
// {
// 	fn eq(&self, other: &Self) -> bool {
// 		// We have `&` access which means
// 		// there is no `&mut` access.
// 		//
// 		// If there is no `&mut` access then
// 		// that means the `Writer`'s `self.remote`
// 		// is equal to whatever `self.arc.load()`
// 		// would produce, so we can skip this.
// 		//
// 		// self.arc         == other.arc
// 		// self.swapping  == other.reclaiming

// 		self.local       == other.local &&
// 		self.remote      == other.remote &&
// 		self.functions     == other.patches &&
// 		self.functions_old == other.patches_old &&
// 		self.tags        == other.tags
// 	}
// }

// impl<T> Default for Writer<T>
// where
// 	T: Clone + Default,
// {
// 	/// Only generates the [`Writer`].
// 	///
// 	/// This initializes your data `T` with [`Default::default()`].
// 	///
// 	/// ```rust
// 	/// # use someday::*;
// 	/// let (_, w1) = someday::default::<usize, PatchUsize>();
// 	/// let w2      = Writer::<usize, PatchUsize>::default();
// 	///
// 	/// assert_eq!(w1, w2);
// 	/// ```
// 	fn default() -> Self {
// 		let local: CommitOwned<T>  = CommitOwned { timestamp: 0, data: Default::default() };
// 		let remote = Arc::new(local.clone());
// 		let arc    = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));
// 		let swapping = Arc::new(AtomicBool::new(false));

// 		let writer = Writer {
// 			local: Some(local),
// 			remote,
// 			arc,
// 			patches: Vec::with_capacity(INIT_VEC_LEN),
// 			patches_old: Vec::with_capacity(INIT_VEC_LEN),
// 			swapping,
// 			tags: BTreeMap::new(),
// 		};

// 		writer
// 	}
// }

// impl<T, Patch> std::ops::Deref for Writer<T>
// where
// 	T: Apply<Patch>,
// {
// 	type Target = T;

// 	fn deref(&self) -> &Self::Target {
// 		&self.local_ref().data
// 	}
// }

// impl<T, Patch> Borrow<T> for Writer<T>
// where
// 	T: Apply<Patch>,
// {
// 	fn borrow(&self) -> &T {
// 		&self.local_ref().data
// 	}
// }

// impl<T, Patch> AsRef<T> for Writer<T>
// where
// 	T: Apply<Patch>,
// {
// 	fn as_ref(&self) -> &T {
// 		&self.local_ref().data
// 	}
// }