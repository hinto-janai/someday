//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::snapshot::{
	Snapshot,SnapshotOwned
};

//---------------------------------------------------------------------------------------------------- Reader
///
#[derive(Clone,Debug)]
pub struct Reader<T> {
	pub(super) arc: Arc<arc_swap::ArcSwap<SnapshotOwned<T>>>,
}

impl<T> Reader<T> {
	#[inline]
	///
	pub fn snapshot(&self) -> Snapshot<T> {
		// May be slower for readers,
		// although, more maybe better
		// to prevent writer starvation.
		// let arc = self.arc.load_full();

		// Faster for readers.
		// May cause writer starvation
		// (writer will clone all the
		// time because there are always
		// strong arc references).
		Snapshot {
			inner: arc_swap::Guard::into_inner(self.arc.load()),
		}
	}

	#[inline]
	///
	pub fn is_behind(&self, snapshot: Snapshot<T>) -> bool {
		self.snapshot().timestamp() > snapshot.timestamp()
	}

	///
	pub fn snapshot_owned(&self) -> SnapshotOwned<T> where T: Clone {
		let arc = arc_swap::Guard::into_inner(self.arc.load());
		match Arc::try_unwrap(arc) {
			Ok(i) => i,
			Err(arc) => SnapshotOwned {
				timestamp: arc.timestamp,
				data: arc.data.clone()
			},
		}
	}
}