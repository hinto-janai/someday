//---------------------------------------------------------------------------------------------------- Use
use std::{sync::Arc, time::Duration};
use crate::{
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	apply::Apply,
	Timestamp,
};

//---------------------------------------------------------------------------------------------------- Writer
/// The single [`Writer`] of some data `T`
///
/// ## Usage
/// This example covers the typical usage of a `Writer`:
/// - Creating some [`Reader`]'s
/// - Adding some `Patch`'s
/// - Viewing the staged `Patch`'s, modifying them
/// - Commiting those changes
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
/// let patches_applied = w.commit();
/// // We applied 3 patches in total.
/// assert_eq!(patches_applied, 3);
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
/// let commits_pushed = w.push();
/// // We pushed 1 commit in total.
/// assert_eq!(commits_pushed, 1);
/// // Our staged patches are now gone.
/// assert_eq!(w.staged().len(), 0);
///
/// // The Readers are now in sync.
/// assert_eq!(w.timestamp(), 1);
/// assert_eq!(r.timestamp(), 1);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head(), "abcdefghi");
/// ```
pub struct Writer<T, Patch>
where
	T: Apply<Patch>,
{
	// The writer's local mutually
	// exclusive copy of the data.
	//
	// This is an `Option` only because there's
	// a brief moment in `commit()` where we need
	// to send off `local`, but we can't yet swap it
	// with the old data.
	//
	// It will be `None` in-between those moments and
	// the invariant is that is MUST be `Some` before
	// `commit()` is over.
	//
	// In release builds `.unwrap_unchecked()` will be used.
	//
	// MaybeUninit probably works clippy is sending me spooky lints.
	pub(super) local: Option<CommitOwned<T>>,

	// The current data the remote `Reader`'s can see.
	pub(super) remote: Arc<CommitOwned<T>>,

	// The AtomicPtr that `Reader`'s enter through.
	// Calling `.load()` would load the `remote` above.
	pub(super) arc: Arc<arc_swap::ArcSwap<CommitOwned<T>>>,

	// Patches that haven't been applied yet.
	pub(super) patches: Vec<Patch>,

	// Patches that were already applied,
	// that must be re-applied to the old `T`.
	pub(super) patches_old: Vec<Patch>,
}

//---------------------------------------------------------------------------------------------------- Writer
impl<T, Patch> Writer<T, Patch>
where
	T: Apply<Patch>,
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
		Reader { arc: Arc::clone(&self.arc) }
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
	/// w.add_and(PatchUsize::Add(1)).commit();
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
	/// w.add_and(PatchUsize::Add(1)).commit();
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
	/// w.add_and(PatchUsize::Add(1)).commit();
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
	/// w.add_and(PatchUsize::Add(1)).commit_and().push();
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
	pub fn add(&mut self, patch: Patch) {
		self.patches.push(patch);
	}

	#[inline]
	///
	pub fn add_and(&mut self, patch: Patch) -> &mut Self {
		self.patches.push(patch);
		self
	}

	#[inline]
	///
	pub fn add_iter(&mut self, patches: impl Iterator<Item = Patch>) {
		for patch in patches {
			self.patches.push(patch);
		}
	}

	#[inline]
	///
	pub fn add_iter_and(&mut self, patches: impl Iterator<Item = Patch>) -> &mut Self {
		for patch in patches {
			self.patches.push(patch);
		}
		self
	}

	#[inline]
	/// Apply a `Patch` to your data, `T`
	///
	/// This immediately calls [`Apply::apply`] with
	/// your patch `Patch` onto your data `T`.
	///
	/// The new [`CommitRef`] created will become
	/// this [`Writer`]'s new [`Writer::head()`].
	pub fn commit(&mut self) -> usize {
		self.commit_inner()
	}

	#[inline]
	/// Apply a `Patch` to your data, `T`
	///
	/// This function is the same as [`Writer::commit()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn commit_and(&mut self) -> &mut Self {
		self.commit_inner();
		self
	}

	fn commit_inner(&mut self) -> usize {
		let patch_len = self.patches.len();

		for mut patch in self.patches.drain(..) {
			Apply::apply(
				&mut patch,
				&mut Self::local_field(&mut self.local).data,
				&self.remote.data,
			);
			self.patches_old.push(patch);
		}

		self.local().timestamp += 1;

		patch_len
	}

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
	/// 	// won't happen, not yet commited
	/// 	unreachable!();
	/// 	// this call would be wasteful
	/// 	w.push();
	/// }
	///
	/// // Now there are commits to push.
	/// w.commit();
	///
	/// if w.ahead() {
	/// 	let commits_pushed = w.push();
	/// 	assert_eq!(commits_pushed, 1);
	/// } else {
	/// 	// won't happen
	/// 	unreachable!();
	/// }
	/// ```
	pub fn push(&mut self) -> usize {
		self.push_inner::<false>(None).0
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_and(&mut self) -> &mut Self {
		self.push_inner::<false>(None);
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
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	/// w.add(PatchString::PushStr("abc".into()));
	/// w.commit();
	///
	/// let commit = r.head();
	/// spawn(move || {
	/// 	// This `Reader` is holding onto the old data.
	/// 	let moved = commit;
	/// 	// But will let go after 1 millisecond.
	/// 	sleep(Duration::from_millis(1));
	/// });
	///
	/// // Wait 10 milliseconds before resorting to cloning data.
	/// let (commits_pushed, reclaimed) = w.push_wait(Duration::from_millis(10));
	/// // We pushed 1 commit.
	/// assert_eq!(commits_pushed, 1);
	/// // And we successfully reclaimed the old data cheaply.
	/// assert_eq!(reclaimed, true);
	/// ```
	pub fn push_wait(&mut self, duration: Duration) -> (usize, bool) {
		self.push_inner::<false>(Some(duration))
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
	/// let commits_pushed = w.push_clone();
	/// // We pushed 1 commit.
	/// assert_eq!(commits_pushed, 1);
	/// ```
	pub fn push_clone(&mut self) -> usize {
		self.push_inner::<true>(None).0
	}

	#[inline]
	/// This function is the same as [`Writer::push_clone()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn push_clone_and(&mut self) -> &mut Self {
		self.push_inner::<true>(None);
		self
	}

	fn push_inner<const CLONE: bool>(&mut self, duration: Option<Duration>) -> (usize, bool) {
		let timestamp_diff    = self.timestamp_diff();
		let current_timestamp = self.timestamp();

		// SAFETY: we're temporarily "taking" our `self.local`.
		// It will be unintialized for the time being.
		// We need to initialize it before returning.
		let local = self.local_take();

		// Swap the reader's `arc_swap` with our new local.
		let local = Arc::new(local);
		self.remote  = Arc::clone(&local);

		// This is the old data from the old AtomicPtr.
		let old = self.arc.swap(Arc::clone(&self.remote));

		// Re-acquire a local copy of data.

		// Return early if the user wants to deep-clone no matter what.
		if CLONE {
			self.local = Some((*self.remote).clone());
			self.local().timestamp = current_timestamp;
			self.patches_old.clear();
			return (timestamp_diff, false);
		}

		// Try to reclaim data.
		let mut reclaimed = false;
		let mut local = match Arc::try_unwrap(old) {
			// If there are no more dangling readers on the
			// old Arc we can cheaply reclaim the old data.
			Ok(old) => {
				reclaimed = true;
				old
			},

			// Else, if the user wants to
			// sleep and try again, do so.
			Err(old) => {
				if let Some(duration) = duration {
					// Sleep.
					std::thread::sleep(duration);
					// Try again.
					if let Some(old) = Arc::into_inner(old) {
						reclaimed = true;
						old
					} else {
						(*self.remote).clone()
					}
				} else {
					// Else, there are dangling readers left.
					// As to not wait on them, just expensively clone
					// the inner data to have a mutually exclusive
					// up-to-date local copy.
					(*self.remote).clone()
				}
			},
		};

		// Re-apply patchs to this old data.
		for mut patch_old in self.patches_old.drain(..) {
			Apply::apply(&mut patch_old, &mut local.data, &self.remote.data);
		}

		// Re-initialize `self.local`.
		self.local = Some(local);

		// Set proper timestamp (we cloned reader's).
		self.local().timestamp = current_timestamp;

		// Return how many commits we pushed.
		(timestamp_diff, reclaimed)
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
	/// w.add_and(PatchString::PushStr("hello".into())).commit();
	/// assert_eq!(w.head(), "hello");
	///
	/// // Reader's sees nothing
	/// assert_eq!(r.head(), "");
	///
	/// // Pull from the Reader.
	/// let old_writer_data = w.pull();
	/// assert_eq!(old_writer_data, "hello");
	///
	///	// We're back to square 1.
	/// assert_eq!(w.head(), "");
	/// ```
	pub fn pull(&mut self) -> CommitOwned<T> {
		self.pull_inner()
	}

	#[inline]
	/// This function is the same as [`Writer::pull()`]
	/// but it returns the [`Writer`] back for method chaining.
	pub fn pull_and(&mut self) -> &mut Self {
		drop(self.pull_inner());
		self
	}

	#[inline]
	fn pull_inner(&mut self) -> CommitOwned<T> {
		// Delete old patches, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.patches_old.clear();
		self.local_swap((*self.remote).clone())
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
	/// 	.add_and(PatchString::PushStr("hello".into()))
	/// 	.commit_and()
	/// 	.push(); // <- commit 1
	///
	/// assert_eq!(w.timestamp(), 1);
	///
	/// // Reader's sees them.
	/// assert_eq!(r.head(), "hello");
	/// assert_eq!(r.timestamp(), 1);
	///
	/// // Commit some changes.
	/// w.add_and(PatchString::Set("hello".into())).commit(); // <- commit 2
	/// w.add_and(PatchString::Set("hello".into())).commit(); // <- commit 3
	/// w.add_and(PatchString::Set("hello".into())).commit(); // <- commit 4
	/// assert_eq!(w.committed_patches().len(), 3);
	///
	/// // Overwrite the Writer with arbitrary data.
	/// let old_data = w.overwrite(String::from("world")); // <- commit 5
	/// assert_eq!(old_data, "hello");
	/// // Commited patches were deleted.
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
		drop(self.overwrite_inner(data));
		self
	}

	#[inline(always)]
	// `T` might be heavy to stack copy, so inline this.
	fn overwrite_inner(&mut self, data: T) -> CommitOwned<T> {
		// Delete old patches, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.patches_old.clear();
		self.local_swap(CommitOwned { timestamp: self.timestamp() + 1, data })
	}

	#[inline]
	/// If the [`Writer`]'s [`Commit`] is different than the [`Reader`]'s
	///
	/// Compares the [`Commit`] that the [`Reader`]'s can
	/// currently access with the [`Writer`]'s current local [`Commit`].
	///
	/// This returns `true` if both:
	/// - The data is different
	/// - The [`Timestamp`] is different
	///
	/// Note that this includes non-[`push()`]'ed [`Writer`] data.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit but don't push.
	/// w.add_and(PatchString::PushStr("abc".into())).commit();
	///
	/// // Writer and Reader's commit is different.
	/// assert!(w.diff());
	/// ```
	pub fn diff(&self) -> bool where T: PartialEq<T> {
		self.local_ref().diff(&*self.remote)
	}

	#[inline]
	/// If the [`Writer`]'s [`Timestamp`] is greater than the [`Reader`]'s [`Timestamp`]
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
	/// 	w.add_and(PatchString::PushStr("abc".into())).commit();
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
	/// If the [`Writer`]'s [`Timestamp`] is greater than an arbitrary [`Commit`]'s [`Timestamp`]
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
	/// 	w.add_and(PatchString::PushStr("abc".into())).commit();
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
	/// If the [`Writer`]'s [`Timestamp`] is less than an arbitrary [`Commit`]'s [`Timestamp`]
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
	/// This number starts at `0`, increments by `1` every time a [`commit()`]
	/// -like operation is called, and it will never be less than the [`Reader`]'s
	/// timestamp.
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
	/// w.add_and(PatchString::PushStr("abc".into())).commit();
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
	/// w.add_and(PatchString::PushStr("abc".into())).commit();
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
	/// w.add_and(PatchString::PushStr("abc".into())).commit_and().push();
	///
	/// // Commit 5 changes locally.
	/// for i in 0..5 {
	/// 	w.add_and(PatchString::PushStr("abc".into())).commit();
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

	/// Restore all the staged changes
	///
	/// This removes all the `Patch`'s that haven't yet been [`commit()`]'ed.
	///
	/// Calling `staged().clear()` would be equivalent.
	///
	/// This function returns how many `Patch`'s were removed.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Add some changes, but don't commit.
	/// w.add(PatchString::PushStr("abc".into()));
	/// assert_eq!(w.staged().len(), 1);
	///
	///	// Restore changes.
	/// let removed = w.restore();
	/// assert_eq!(removed, 1);
	/// ```
	pub fn restore(&mut self) -> usize {
		let patch_len = self.patches.len();

		if patch_len != 0 {
			self.patches.clear();
		}

		patch_len
	}

	#[inline]
	/// All the `Patch`'s that **haven't** been [`commit()`](Writer::commit)'ed yet, aka, "staged" changes
	///
	/// You are allowed to do anything to these `Patch`'s as they haven't
	/// been commited yet and the `Writer` does not necessarily  need them.
	///
	/// You can use something like `.staged().drain(..)` to get back all the `Patch`'s.
	///
	/// All the `Patch`'s that have been [`commit()`](Writer::commit)'ed but not yet
	/// [`push()`](Writer::push)'ed are safely stored internally by the [`Writer`].
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Add some changes.
	/// let change = PatchString::PushStr("abc".into());
	/// w.add(change.clone());
	///
	/// // We see and mutate the staged changes.
	/// assert_eq!(w.staged().len(), 1);
	/// assert_eq!(w.staged()[0], change);
	///
	/// // Let's actually remove that change.
	/// let removed = w.staged().remove(0);
	/// assert_eq!(w.staged().len(), 0);
	/// assert_eq!(change, removed);
	/// ```
	pub fn staged(&mut self) -> &mut Vec<Patch> {
		&mut self.patches
	}

	#[inline]
	/// All the `Patch`'s that **have** been [`commit()`](Writer::commit)'ed but not yet [`push()`](Writer::push)'ed
	///
	/// You are not allowed to mutate these `Patch`'s as they haven't been
	/// [`push()`](Writer::push)'ed yet and the `Writer` may need them in the future.
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit some changes.
	/// let change = PatchString::PushStr("abc".into());
	/// w.add(change.clone());
	/// w.commit();
	///
	/// // We can see but not mutate patches.
	/// assert_eq!(w.committed_patches().len(), 1);
	/// assert_eq!(w.committed_patches()[0], change);
	/// ```
	pub fn committed_patches(&self) -> &Vec<Patch> {
		&self.patches_old
	}

	/// Consume this [`Writer`] and return the inner components
	///
	/// In left-to-right order, this returns:
	/// 1. The [`Writer`]'s local data
	/// 2. The latest [`Reader`]'s [`Commit`] (aka, from [`Reader::head()`])
	/// 3. The "staged" `Patch`'s that haven't been [`commit()`](Writer::commit)'ed (aka, from [`Writer::staged()`])
	/// 4. The commited `Patch`'s that haven't been [`push()`](Writer::push)'ed (aka, from [`Writer::commited_patches()`])
	///
	/// ```rust
	/// # use someday::{*,patch::*};
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Commit some changes.
	/// let committed_change = PatchString::PushStr("a".into());
	/// w.add(committed_change.clone());
	/// w.commit();
	///
	/// // Add but don't commit
	/// let staged_change = PatchString::PushStr("b".into());
	/// w.add(staged_change.clone());
	///
	/// let (
	/// 	writer_data,
	/// 	reader_data,
	/// 	staged_changes,
	/// 	committed_changes,
	/// ) = w.into_inner();
	///
	/// assert_eq!(writer_data, "a");
	/// assert_eq!(reader_data, ""); // We never `push()`'ed, so Readers saw nothing.
	/// assert_eq!(staged_changes[0], staged_change);
	/// assert_eq!(committed_changes[0], committed_change);
	/// ```
	pub fn into_inner(mut self) -> (CommitOwned<T>, CommitRef<T>, Vec<Patch>, Vec<Patch>) {
		let local = self.local_take();

		let snap = CommitOwned {
			timestamp: local.timestamp,
			data: local.data,
		};

		(snap, CommitRef { inner: self.remote }, self.patches, self.patches_old)
	}
}

//---------------------------------------------------------------------------------------------------- Private writer functions
impl<T, Patch> Writer<T, Patch>
where
	T: Apply<Patch>,
{
	// HACK:
	// These `local_*()` functions are a work around.
	// Writer's local data is almost always initialized, but
	// during `commit()` there's a brief moment where we send our
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
}

//---------------------------------------------------------------------------------------------------- Writer trait impl
impl<T, Patch> std::fmt::Debug for Writer<T, Patch>
where
	T: Apply<Patch> + std::fmt::Debug,
	Patch: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CommitOwned")
			.field("local", &self.local)
			.field("arc", &self.arc)
			.field("remote", &self.remote)
			.field("patches", &self.patches)
			.finish()
	}
}

impl<T, Patch> Default for Writer<T, Patch>
where
	T: Apply<Patch> + Default,
{
	fn default() -> Self {
		let local = CommitOwned { timestamp: 0, data: T::default() };
		let remote   = Arc::new(local.clone());
		let arc   = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&remote)));

		use crate::INIT_VEC_LEN;

		let writer = Writer {
			local: Some(local),
			arc,
			remote,
			patches: Vec::with_capacity(INIT_VEC_LEN),
			patches_old: Vec::with_capacity(INIT_VEC_LEN),
		};

		writer
	}
}

impl<T, Patch> std::ops::Deref for Writer<T, Patch>
where
	T: Apply<Patch>,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.local_ref().data
	}
}

impl<T, Patch> AsRef<T> for Writer<T, Patch>
where
	T: Apply<Patch>,
{
	fn as_ref(&self) -> &T {
		&self.local_ref().data
	}
}