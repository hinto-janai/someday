//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	reader::Reader,
	snapshot::{Snapshot,SnapshotOwned},
	ops::Operation,
};

//---------------------------------------------------------------------------------------------------- Writer
///
pub struct Writer<T, O>
where
	T: Clone + Operation<O>,
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
	pub(super) local: Option<SnapshotOwned<T>>,

	// pub(super) cfg: Config,

	// The AtomicPtr that reader's enter through.
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<SnapshotOwned<T>>>>,

	// The current Arc stored in the above pointer.
	pub(super) now: Arc<SnapshotOwned<T>>,

	// The functions to apply to `T`.
	pub(super) ops: Vec<O>,
}

//---------------------------------------------------------------------------------------------------- Writer
impl<T, O> Writer<T, O>
where
	T: Clone + Operation<O>,
{
	#[inline]
	///
	pub fn apply(&mut self, mut operation: O) -> &mut Self {
		let local = self.local();
		local.timestamp += 1;
		Operation::apply(&mut local.data, &mut operation);
		self.ops.push(operation);
		self
	}

	#[inline]
	///
	pub fn commit(&mut self) -> usize {
		if self.local().timestamp == self.now.timestamp {
			return 0;
		}

		let ops_len = self.ops.len();

		if ops_len == 0 {
			return 0;
		}

		// SAFETY: we're temporarily "taking" our `self.local`.
		// It will be unintialized for the time being.
		// We need to initialize it before returning.
		let local = self.local_take();

		// Swap the reader's `arc_swap` with our new local.
		let local   = Arc::new(local);
		let mut now = Arc::clone(&local);
		self.now    = Arc::clone(&local);
		std::mem::swap(&mut self.now, &mut now);

		// This is the old data from the old AtomicPtr.
		let old = self.arc.swap(now);

		// If there are no more dangling readers on the
		// old Arc we can cheaply reclaim the old data.
		let mut local = if let Some(old) = Arc::into_inner(old) {
			old
		} else {
			// Else, there are dangling readers left.
			// As to not wait on them, just expensively clone
			// the inner data to have a mutually exclusive
			// up-to-date local copy.
			(*self.now).clone()
		};

		// Re-apply operations to this old data.
		for op in self.ops.iter_mut() {
			Operation::apply(&mut local.data, op);
		}

		// Re-initialize `self.local`.
		self.local = Some(local);

		// Clear the operations.
		self.ops.clear();

		// Return how many operations we commited.
		ops_len
	}

	#[inline]
	///
	pub fn reader(&self) -> Reader<T> {
		Reader { arc: Arc::clone(&self.arc) }
	}

	#[inline]
	///
	pub fn read(&self) -> &T {
		&self.local_ref().data
	}

	#[inline]
	///
	pub fn snapshot(&self) -> Snapshot<T> {
		Snapshot { inner: Arc::clone(&self.now) }
	}

	///
	pub fn snapshot_owned(&self) -> SnapshotOwned<T> {
		// We are the writer, so we know we cannot
		// `Arc::into_inner()`, so clone the data.
		SnapshotOwned {
			timestamp: self.now.timestamp,
			data: self.now.data.clone()
		}
	}

	#[inline]
	///
	pub fn is_ahead(&self) -> bool {
		self.local_ref().timestamp > self.now.timestamp
	}

	#[inline]
	///
	pub fn operations(&self) -> &[O] {
		&self.ops
	}

	///
	pub fn into_inner(mut self) -> (SnapshotOwned<T>, Vec<O>) {
		let local = self.local_take();

		let snap = SnapshotOwned {
			timestamp: local.timestamp,
			data: local.data,
		};

		(snap, self.ops)
	}

	#[inline]
	///
	pub fn timestamp_writer(&self) -> usize {
		self.local_ref().timestamp
	}

	#[inline]
	///
	pub fn timestamp_reader(&self) -> usize {
		self.now.timestamp
	}

	#[inline]
	///
	pub fn timestamp_diff(&self) -> usize {
		self.local_ref().timestamp - self.now.timestamp
	}

	#[inline]
	///
	pub fn timestamp_after_commit(&self) -> usize {
		self.local_ref().timestamp + self.ops.len()
	}

	#[inline]
	///
	pub fn timestamp_after_commit_diff(&self) -> usize {
		self.timestamp_after_commit() - self.now.timestamp
	}
}

//---------------------------------------------------------------------------------------------------- Private writer functions
impl<T, O> Writer<T, O>
where
	T: Clone + Operation<O>,
{
	#[inline(always)]
	fn local(&mut self) -> &mut SnapshotOwned<T> {
		#[cfg(debug_assertions)]
		{ self.local.as_mut().unwrap() }

		#[cfg(not(debug_assertions))]
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.as_mut().unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_take(&mut self) -> SnapshotOwned<T> {
		#[cfg(debug_assertions)]
		{ self.local.take().unwrap() }

		#[cfg(not(debug_assertions))]
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.take().unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_inner(self) -> SnapshotOwned<T> {
		#[cfg(debug_assertions)]
		{ self.local.unwrap() }

		#[cfg(not(debug_assertions))]
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.unwrap_unchecked() }
	}

	#[inline(always)]
	fn local_ref(&self) -> &SnapshotOwned<T> {
		#[cfg(debug_assertions)]
		{ self.local.as_ref().unwrap() }

		#[cfg(not(debug_assertions))]
		// SAFETY: This is always initialized with something.
		// When it isn't (`commit()`), this function isn't used.
		unsafe { self.local.as_ref().unwrap_unchecked() }
	}
}

//---------------------------------------------------------------------------------------------------- Writer trait impl
impl<T, O> std::fmt::Debug for Writer<T, O>
where
	T: Clone + Operation<O> + std::fmt::Debug,
	O: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SnapshotOwned")
			.field("local", &self.local)
			.field("arc", &self.arc)
			.field("now", &self.now)
			.field("ops", &self.ops)
			.finish()
	}
}

impl<T, O> Default for Writer<T, O>
where
	T: Clone + Operation<O> + Default,
{
	fn default() -> Self {
		let local = SnapshotOwned { timestamp: 0, data: T::default() };
		let now   = Arc::new(local.clone());
		let arc   = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&now)));

		let writer = Writer {
			local: Some(local),
			arc,
			now,
			ops: vec![],
		};

		writer
	}
}

impl<T, O> std::ops::Deref for Writer<T, O>
where
	T: Clone + Operation<O>,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.local_ref().data
	}
}

impl<T, O> AsRef<T> for Writer<T, O>
where
	T: Clone + Operation<O>,
{
	fn as_ref(&self) -> &T {
		&self.local_ref().data
	}
}

impl<T, O> From<T> for Writer<T, O>
where
	T: Clone + Operation<O>,
{
	fn from(data: T) -> Self {
		let local = SnapshotOwned { timestamp: 0, data };
		let now   = Arc::new(local.clone());
		let arc   = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&now)));

		let writer = Writer {
			local: Some(local),
			arc,
			now,
			ops: vec![],
		};

		writer
	}
}