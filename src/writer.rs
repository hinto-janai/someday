//---------------------------------------------------------------------------------------------------- Use
use std::{sync::Arc, path::Iter};
use crate::{
	reader::Reader,
	snapshot::{Snapshot,SnapshotOwned},
	ops::Operation,
};
use std::collections::VecDeque;

//---------------------------------------------------------------------------------------------------- Writer
///
#[derive(Debug)]
pub struct Writer<T, O>
where
	T: Clone + Operation<O>,
{
	// The writer's local mutually
	// exclusive copy of the data.
	pub(super) local: SnapshotOwned<T>,
	// The AtomicPtr that reader's enter through.
	pub(super) arc: Arc<arc_swap::ArcSwap<SnapshotOwned<T>>>,
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
		self.local.timestamp += 1;
		Operation::apply(&mut self.local.data, &mut operation);
		self.ops.push(operation);
		self
	}

	#[inline]
	///
	pub fn read(&self) -> &T {
		&self.local.data
	}

	#[inline]
	///
	pub fn reader(&self) -> Reader<T> {
		Reader { arc: Arc::clone(&self.arc) }
	}

	#[inline]
	///
	pub fn snapshot(&self) -> Snapshot<T> {
		Snapshot { inner: Arc::clone(&self.now) }
	}

	///
	pub fn into_inner(self) -> SnapshotOwned<T> {
		SnapshotOwned {
			timestamp: self.local.timestamp,
			data: self.local.data,
		}
	}

	///
	pub fn snapshot_owned(&self) -> SnapshotOwned<T> where T: Clone {
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
		self.local.timestamp > self.now.timestamp
	}

	#[inline]
	///
	pub fn timestamp_writer(&self) -> usize {
		self.local.timestamp
	}

	#[inline]
	///
	pub fn timestamp_reader(&self) -> usize {
		self.now.timestamp
	}

	#[inline]
	///
	pub fn timestamp_diff(&self) -> usize {
		self.local.timestamp - self.now.timestamp
	}

	#[inline]
	///
	pub fn timestamp_after_commit(&self) -> usize {
		self.local.timestamp + self.ops.len()
	}

	#[inline]
	///
	pub fn timestamp_after_commit_diff(&self) -> usize {
		self.timestamp_after_commit() - self.now.timestamp
	}

	#[inline]
	///
	pub fn operation_len(&self) -> usize {
		self.ops.len()
	}

	#[inline]
	///
	pub fn restore(&mut self) -> usize {
		let ops = self.ops.len();
		if ops > 0 {
			self.local = (*self.now).clone();
			self.ops.clear();
			ops
		} else {
			0
		}
	}

	#[inline]
	///
	pub fn commit(&mut self) -> usize {
		if self.local.timestamp == self.now.timestamp {
			return 0;
		}

		let ops_len = self.ops.len();

		if ops_len == 0 {
			return 0;
		}

		// SAFETY: we're temporarily "taking" our `self.local`.
		// It will be unintialized for the time being.
		// We need to initialize it before returning.
		let mut local = unsafe { std::mem::MaybeUninit::<SnapshotOwned<T>>::uninit().assume_init() };
		std::mem::swap(&mut self.local, &mut local);

		// Swap the reader's `arc_swap` with our new local.
		let local    = Arc::new(local);
		let mut now  = Arc::clone(&local);
		self.now     = Arc::clone(&local);
		std::mem::swap(&mut self.now, &mut now);

		let old = self.arc.swap(now);

		// And there are no more dangling
		// readers on the old Arc...
		if let Some(old) = Arc::into_inner(old) {
			// Then we can reclaim the old data as our local.
			// This is fine because:
			// 1. There are no more readers, this data is mutually exclusive
			// 2. The data is the exact same as the readers data, so we're up-to-date
			//
			// SAFETY: we're re-initializing the uninitialized `self.local`.
			unsafe { std::ptr::addr_of_mut!(self.local).write(old); }
		} else {
			// Else, there are dangling readers left.
			// To not wait on them, just expensively clone
			// the inner data to have a mutually exclusive
			// up-to-date local copy.
			//
			// SAFETY: we're re-initializing the uninitialized `self.local`.
			unsafe { std::ptr::addr_of_mut!(self.local).write((*self.now).clone()); }
		}

		for op in self.ops.iter_mut() {
			Operation::apply(&mut self.local.data, op);
		}

		self.ops.clear();

		ops_len
	}

}