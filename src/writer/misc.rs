//! `Writer<T>`

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
	collections::BTreeMap,
	num::NonZeroUsize,
};

use crate::{
	writer::Writer,
	patch::Patch,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
	/// Same as [`crate::free::new`] but without creating a [`Reader`].
	///
	/// TODO: doc test.
	pub fn new(data: T) -> Self {
		crate::free::new_inner(CommitOwned { data, timestamp: 0 })
	}
	#[inline]
	#[allow(clippy::type_complexity)]
	/// Restore all the staged changes.
	///
	/// This removes all the `Patch`'s that haven't yet been [`commit()`](Writer::commit)'ed.
	///
	/// Calling `Writer::staged().drain(..)` would be equivalent.
	///
	/// Dropping the [`std::vec::Drain`] will drop the `Patch`'s.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Add some changes, but don't commit.
	/// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
	/// assert_eq!(w.staged().len(), 1);
	///
	/// // Restore changes.
	/// let drain = w.restore();
	/// assert_eq!(drain.count(), 1);
	/// ```
	pub fn restore(&mut self) -> std::vec::Drain<'_, Patch<T>> {
		self.patches.drain(..)
	}

	/// Shrinks the capacity of the `Patch` [`Vec`]'s as much as possible
	///
	/// This calls [`Vec::shrink_to_fit()`] on the 2
	/// internal `Vec`'s in [`Writer`] holding:
	/// 1. The currently staged `Patch`'s
	/// 2. The already committed `Patch`'s
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // Capacity is 16.
	/// assert_eq!(w.committed_patches().capacity(), 16);
	/// assert_eq!(w.staged().capacity(),            16);
	///
	/// // Commit 32 `Patch`'s
	/// for i in 0..32 {
	///     w.add(Patch::Ptr(|w, _| *w = "".into()));
	///     w.commit();
	/// }
	/// // Stage 16 `Patch`'s
	/// for i in 0..16 {
	///     w.add(Patch::Ptr(|w, _| *w = "".into()));
	/// }
	///
	/// // Commit capacity is now 32.
	/// assert_eq!(w.committed_patches().capacity(), 32);
	/// // This didn't change, we already had
	/// // enough space to store them.
	/// assert_eq!(w.staged().capacity(), 16);
	///
	/// // Commit, push, shrink.
	/// w.commit();
	/// w.push();
	/// w.shrink_to_fit();
	///
	/// // They're now empty and taking 0 space.
	/// assert_eq!(w.committed_patches().capacity(), 0);
	/// assert_eq!(w.staged().capacity(), 0);
	/// ```
	pub fn shrink_to_fit(&mut self) {
		self.patches.shrink_to_fit();
		self.patches_old.shrink_to_fit();
	}

	/// Reserve capacity in the `Patch` [`Vec`]'s
	///
	/// This calls [`Vec::reserve_exact()`] on the 2
	/// internal `Vec`'s in [`Writer`] holding:
	/// 1. The currently staged `Patch`'s
	/// 2. The already committed `Patch`'s
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // Capacity is 16.
	/// assert_eq!(w.committed_patches().capacity(), 16);
	/// assert_eq!(w.staged().capacity(),            16);
	///
	/// // Reserve space for 48 more patches.
	/// w.reserve_exact(48);
	/// assert!(w.committed_patches().capacity() >= 48);
	/// assert!(w.staged().capacity()            >= 48);
	/// ```
	///
	/// # Panics
	/// Panics if the new capacity exceeds [`isize::MAX`] bytes.
	pub fn reserve_exact(&mut self, additional: usize) {
		self.patches.reserve_exact(additional);
		self.patches_old.reserve_exact(additional);
	}

	/// Is this [`Writer`] associated with this [`Reader`]?
	///
	/// This returns `true` if both `self` and `other` are `Reader`'s from the same `Writer`.
	///
	/// This means both `Reader`'s receive the same [`Commit`] upon calling [`Reader::head`].
	///
	/// TODO: doc test.
	pub fn connected(&self, reader: &Reader<T>) -> bool {
		Arc::ptr_eq(&self.arc, &reader.arc)
	}

	/// Disconnect from the [`Reader`]'s associated with this [`Writer`].
	///
	/// This completely severs the link between the
	/// `Reader`'s associated with this `Writer`.
	///
	/// Any older `Reader`'s will no longer receive [`Commit`]'s
	/// from this `Writer`, and [`Reader::writer_dead`] will start
	/// to return `true`. From the perspective of the older `Reader`'s,
	/// calling this function is the same as this `Writer` being dropped.
	///
	/// Any future `Reader`'s created after this function
	/// are completely separate from the past `Reader`'s.
	///
	/// TODO: doc test.
	pub fn disconnect(&mut self) {
		let token = Arc::new(AtomicBool::new(false));
		let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&self.remote)));

		self.token = token.into();
		self.arc = arc;
	}

	#[allow(clippy::missing_panics_doc, clippy::type_complexity)]
	/// Consume this [`Writer`] and return the inner components.
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Commit some changes.
	/// w.add(Patch::Ptr(|w, _| w.push_str("a")));
	/// w.commit();
	///
	/// // Add but don't commit.
	/// w.add(Patch::Ptr(|w, _| w.push_str("b")));
	///
	/// let WriterInfo {
	///     writer,
	///     reader,
	///     staged,
	///     committed_patches,
	/// } = w.into_inner();
	///
	/// assert_eq!(writer.data(), "a");
	/// assert_eq!(reader.data(), ""); // We never `push()`'ed, so Readers saw nothing.
	/// assert_eq!(staged.len(), 1);
	/// assert_eq!(committed_patches.len(), 1);
	/// ```
	pub fn into_inner(self) -> WriterInfo<T> {
		WriterInfo {
			// INVARIANT: local must be initialized after push()
			writer:	self.local.unwrap(),
			reader:	self.remote,
			staged:	self.patches,
			committed_patches: self.patches_old,
		}
	}
}