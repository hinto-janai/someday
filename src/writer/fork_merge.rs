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
	#[must_use]
	#[allow(clippy::missing_panics_doc)]
	/// Fork off from the current [`Reader::head`] commit and create a [`Writer`].
	///
	/// This function:
	/// - Clones the current latest `Reader` data
	/// - Clones the [`Writer::tags`]
	/// - Creates a completely new `Writer` that is _disconnected_ from this `Reader` (and others)
	///
	/// The new `Writer`:
	/// - will contain no [`Patch`]'s
	/// - is disconnected, meaning it has absolutely no
	/// relation to `self` or any other previous `Reader`'s.
	/// - has the latest `Reader` data as the local, mutable [`Writer::data`]
	///
	/// The [`Commit`]'s pushed by the returned `Writer` will not be visible to previous `Reader`'s
	///
	/// TODO: doc test.
	pub fn fork(&self) -> Self {
		let remote = Arc::clone(&self.remote);
		let local = remote.to_commit_owned();
		let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));

		Self {
			token: Arc::new(AtomicBool::new(false)).into(),
			local: Some(local),
			remote,
			arc,
			patches: Vec::with_capacity(self.patches.capacity()),
			patches_old: Vec::with_capacity(self.patches_old.capacity()),
			tags: self.tags.clone(),
		}
	}

	/// TODO
	///
	/// # Errors
	/// TODO
	#[allow(clippy::missing_panics_doc)]
	pub fn merge<M>(&mut self, mut other: Self, mut merge: M) -> Result<Timestamp, usize>
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

		// If we have tags...
		if let Some(max_entry) = self.tags.last_entry() {
			// And the `other` also had tags...
			if let Some(other_max_entry) = other.tags.last_entry() {
				// Then take all the ones that have a greater timestamp.
				//
				// We must not take the older ones since:
				// 1. They may be different Commits
				// 2. We must uphold the invariant each tag
				//    actually existed at some point in our `self`
				// 3. We must not overwrite older tags

				// Only take if there are greater timestamps in `other.`
				let latest_timestamp = max_entry.key();
				if latest_timestamp < other_max_entry.key() {
					// Take out all the less than timestamps.
					other.tags.retain(|timestamp, _| timestamp < latest_timestamp);
					// And append the rest.
					self.tags.append(&mut other.tags);
				}
			}
		}

		// Take the old patches.
		self.patches_old.extend(other.patches_old);

		Ok(self.timestamp())
	}
}