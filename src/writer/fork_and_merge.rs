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
	/// TODO
	pub fn fork(&self) -> Self {
		let remote = Arc::clone(&self.remote);
		let local = remote.to_commit_owned();
		let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));

		Self {
			local: Some(local),
			remote,
			arc,
			patches: Vec::with_capacity(self.patches.capacity()),
			patches_old: Vec::with_capacity(self.patches_old.capacity()),
			tags: self.tags.clone(),
		}
	}

	// /// TODO
	// pub fn merge(&mut self, mut other: Self) -> Result<(), MergeConflict> {
	// 	if !self.ahead_of(other.local_as_ref()) {
	// 		return;
	// 	}

	// 	self.tags.append(&mut other.tags);
	// }
}