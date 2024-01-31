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
	writer::token::WriterToken,
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
	#[must_use]
	#[allow(clippy::missing_panics_doc)]
	/// Fork off from the current [`Reader::head`] commit and create a [`Writer`].
	///
	/// This new `Writer`:
	/// - will contain no [`Patch`]'s
	/// - is disconnected, meaning it has absolutely no
	/// relation to `self` or any other previous `Reader`'s.
	/// - has the latest [`Writer::head`] as the base for `Writer` and `Reader`'s
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new(String::new());
	///
	/// // Connected `Reader` <-> `Writer`.
	/// assert!(r.connected_writer(&w));
	///
	/// // Add local changes, but don't push.
	/// w.add_commit(|s, _| {
	///     s.push_str("hello");
	/// });
	/// assert_eq!(w.data(), "hello");
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(r.head().data(), "");
	/// assert_eq!(r.head().timestamp(), 0);
	///
	/// // Fork off into another `Writer`.
	/// let mut w2 = w.fork();
	/// let r2 = w2.reader();
	///
	/// // It inherits the data of the previous `Writer`.
	/// assert_eq!(w.data(), "hello");
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(w.head().data(), "hello");
	/// assert_eq!(w.head().timestamp(), 1);
	///
	/// // And has no relation to the previous `Writer/Reader`'s.
	/// assert!(!r.connected(&r2));
	/// assert!(!r.connected_writer(&w2));
	///
	/// w2.add_commit(|s, _| {
	///     s.push_str(" world!");
	/// });
	///
	/// assert_eq!(w2.data(), "hello world!");
	/// assert_eq!(w2.timestamp(), 2);
	/// assert_eq!(w.data(), "hello");
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(r.head().data(), "");
	/// assert_eq!(r.head().timestamp(), 0);
	/// ```
	pub fn fork(&self) -> Self {
		let local = self.local.as_ref().unwrap().clone();
		let remote = Arc::new(local.clone());
		let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));

		Self {
			token: WriterToken::new(),
			local: Some(local),
			remote,
			arc,
			patches: Vec::with_capacity(self.patches.capacity()),
			patches_old: Vec::with_capacity(self.patches_old.capacity()),
		}
	}

	/// TODO
	///
	/// # Errors
	/// TODO
	#[allow(clippy::missing_panics_doc)]
	pub fn merge<M>(&mut self, other: Self, mut merge: M) -> Result<Timestamp, usize>
	where
		T: Send + 'static,
		M: FnMut(&mut T, &T) + Send + 'static,
	{
		// INVARIANT: local should always be initialized.
		let other_local = other.local.unwrap();

		// If timestamp if not greater, return, nothing to merge.
		let timestamp = self.timestamp();
		let timestamp_diff = other_local.timestamp.saturating_sub(timestamp);
		if timestamp_diff == 0 {
			return Err(timestamp - other_local.timestamp);
		}

		// Overwrite our data with `other`'s.
		// let old_writer_commit = self.overwrite(other_local.data);
		merge(
			&mut self.local.as_mut().unwrap().data,
			&other_local.data,
		);

		// Make sure the timestamp is now the new commit's.
		self.local_as_mut().timestamp = other_local.timestamp;

		self.patches_old.push(Patch::boxed(move |w, _| {
			merge(w, &other_local.data);
		}));

		// Take the old patches.
		self.patches_old.extend(other.patches_old);

		Ok(self.timestamp())
	}
}