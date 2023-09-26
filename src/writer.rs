use core::time;
//---------------------------------------------------------------------------------------------------- Use
use std::{sync::Arc, time::Duration};
use crate::{
	reader::Reader,
	commit::{Commit,CommitOwned},
	apply::Apply,
	Timestamp,
};

//---------------------------------------------------------------------------------------------------- Writer
/// The single [`Writer`] of some data `T`
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
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<CommitOwned<T>>>>,

	// Patches (functions) that were already applied,
	// that must be re-applied to the old `T`.
	pub(super) patches: Vec<Patch>,
}

//---------------------------------------------------------------------------------------------------- Writer
impl<T, Patch> Writer<T, Patch>
where
	T: Apply<Patch>,
{
	#[inline]
	///
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
	/// The new [`Commit`] created will become
	/// this [`Writer`]'s new [`Writer::head()`].
	pub fn commit(&mut self) -> usize {
		self.commit_inner()
	}

	#[inline]
	/// Apply a `Patch` to your data, `T`
	///
	/// This function does the same thing as [`Writer::commit()`] but it returns
	/// the `Writer` instead. Useful for chaining multiple calls together.
	pub fn commit_and(&mut self) -> &mut Self {
		self.commit_inner();
		self
	}

	fn commit_inner(&mut self) -> usize {
		let patch_len = self.patches.len();

		// Don't drop the patches yet, we may
		// need to re-apply them to the reclaimed
		// data in `push()`.
		for patch in self.patches.iter() {
			Apply::apply(
				patch,
				&mut Self::local_field(&mut self.local).data,
				&self.remote.data,
			);
		}

		self.local().timestamp += 1;

		patch_len
	}

	#[inline]
	///
	pub fn push(&mut self) -> usize {
		self.push_inner::<false>(None)
	}

	#[inline]
	///
	pub fn push_and(&mut self) -> &mut Self {
		self.push_inner::<false>(None);
		self
	}

	#[inline]
	///
	pub fn push_wait(&mut self, duration: Duration) -> usize {
		self.push_inner::<false>(Some(duration))
	}

	#[inline]
	///
	pub fn push_clone(&mut self) -> usize {
		self.push_inner::<true>(None)
	}

	#[inline]
	///
	pub fn push_clone_and(&mut self) -> &mut Self {
		self.push_inner::<true>(None);
		self
	}

	fn push_inner<const CLONE: bool>(&mut self, duration: Option<Duration>) -> usize {
		if self.patches.len() == 0 {
			return 0;
		}

		let timestamp_diff = self.timestamp_diff();

		if timestamp_diff == 0 {
			return 0;
		}

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
			self.patches.clear();
			return timestamp_diff;
		}

		// Try to reclaim data.
		let mut local = match Arc::try_unwrap(old) {
			// If there are no more dangling readers on the
			// old Arc we can cheaply reclaim the old data.
			Ok(old) => old,

			// Else, if the user wants to
			// sleep and try again, do so.
			Err(old) => {
				if let Some(duration) = duration {
					// Sleep.
					std::thread::sleep(duration);
					// Try again.
					if let Some(old) = Arc::into_inner(old) {
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
		for patch in self.patches.iter_mut() {
			Apply::apply(patch, &mut local.data, &self.remote.data);
		}

		// Re-initialize `self.local`.
		self.local = Some(local);

		// Clear the patchs.
		self.patches.clear();

		// Return how many commits we pushed.
		timestamp_diff
	}

	#[inline]
	///
	pub fn pull(&mut self) {
		self.local = Some((*self.remote).clone());
	}

	#[inline]
	///
	pub fn pull_and(&mut self) -> &mut Self {
		self.local = Some((*self.remote).clone());
		self
	}

	#[inline]
	///
	pub fn rebase(&mut self, data: CommitOwned<T>) {
		self.local = Some(data);
	}

	#[inline]
	///
	pub fn rebase_and(&mut self, data: CommitOwned<T>) -> &mut Self {
		self.local = Some(data);
		self
	}

	#[inline]
	///
	pub fn data(&self) -> &T {
		&self.local_ref().data
	}

	#[inline]
	///
	pub fn reader(&self) -> Reader<T> {
		Reader { arc: Arc::clone(&self.arc) }
	}

	#[inline]
	///
	pub fn head(&self) -> &CommitOwned<T> {
		self.local_ref()
	}

	#[inline]
	///
	pub fn head_remote(&self) -> Commit<T> {
		Commit { inner: Arc::clone(&self.remote) }
	}

	#[inline]
	/// Compares the timestamp of the [`Reader`]'s current available
	/// data with the [`Writer`]'s current local timestamp.
	///
	/// This returns `true` if the [`Writer`]'s timestamp
	/// is greater than [`Reader`]'s timestamp (which means
	/// [`Writer`] is ahead of the [`Reader`]'s)
	///
	/// Note that this includes non-[`commit()`]'ed [`Writer`] data.
	pub fn ahead(&self) -> bool {
		self.local_ref().timestamp > self.remote.timestamp
	}

	#[inline]
	/// Compares `commit`'s timestamp with the [`Writer`]'s current timestamp
	///
	/// This returns `true` if the [`Writer`]'s timestamp
	/// is greater than the [`Commit`]'s timestamp.
	///
	/// Note that this includes non-[`commit()`]'ed [`Writer`] data.
	fn ahead_of(&self, commit: &Commit<T>) -> bool {
		self.local_ref().timestamp > commit.timestamp()
	}

	#[inline]
	///
	pub fn timestamp(&self) -> Timestamp {
		self.local_ref().timestamp
	}

	#[inline]
	///
	pub fn timestamp_reader(&self) -> Timestamp {
		self.remote.timestamp
	}

	#[inline]
	///
	pub fn timestamp_diff(&self) -> usize {
		self.local_ref().timestamp - self.remote.timestamp
	}

	///
	pub fn restore(&mut self) -> usize {
		let patch_len = self.patches.len();

		// Return early if there are no patches.
		if patch_len == 0 {
			return 0;
		}

		self.patches.clear();
		patch_len
	}

	#[inline]
	///
	pub fn patches(&mut self) -> &mut [Patch] {
		&mut self.patches
	}

	///
	pub fn into_inner(mut self) -> (CommitOwned<T>, Commit<T>, Vec<Patch>) {
		let local = self.local_take();

		let snap = CommitOwned {
			timestamp: local.timestamp,
			data: local.data,
		};

		(snap, Commit { inner: self.remote }, self.patches)
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