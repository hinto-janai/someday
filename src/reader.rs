//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	commit::{Commit,CommitOwned},
	Timestamp,
};

//---------------------------------------------------------------------------------------------------- Reader
/// Reader(s) who can atomically read some data `T`
#[derive(Clone,Debug)]
pub struct Reader<T> {
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<CommitOwned<T>>>>,
}

impl<T> Reader<T> {
	#[inline]
	///
	pub fn head(&self) -> Commit<T> {
		// May be slower for readers,
		// although, more maybe better
		// to prevent writer starvation.
		// let arc = self.arc.load_full();

		// Faster for readers.
		// May cause writer starvation
		// (writer will clone all the
		// time because there are always
		// strong arc references).
		Commit {
			inner: arc_swap::Guard::into_inner(self.arc.load()),
		}
	}

	#[inline]
	///
	pub fn is_ahead_of(&self, commit: Commit<T>) -> bool {
		self.head().timestamp() > commit.timestamp()
	}

	#[inline]
	///
	pub fn timestamp(&self) -> Timestamp {
		self.head().timestamp()
	}

	///
	pub fn head_owned(&self) -> CommitOwned<T> where T: Clone {
		let arc = arc_swap::Guard::into_inner(self.arc.load());
		match Arc::try_unwrap(arc) {
			Ok(i) => i,
			Err(arc) => CommitOwned {
				timestamp: arc.timestamp,
				data: arc.data.clone()
			},
		}
	}
}