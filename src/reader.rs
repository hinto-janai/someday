//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	Writer,
	Apply,
};

//---------------------------------------------------------------------------------------------------- Reader
/// Reader(s) who can atomically read some data `T`
#[derive(Clone,Debug)]
pub struct Reader<T>
where
	T: Clone,
{
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<CommitOwned<T>>>>,
}

impl<T> Reader<T>
where
	T: Clone,
{
	#[inline]
	///
	pub fn head(&self) -> CommitRef<T> {
		// May be slower for readers,
		// although, more maybe better
		// to prevent writer starvation.
		// let arc = self.arc.load_full();

		// Faster for readers.
		// May cause writer starvation
		// (writer will clone all the
		// time because there are always
		// strong arc references).
		CommitRef {
			inner: arc_swap::Guard::into_inner(self.arc.load()),
		}
	}

	#[inline]
	///
	pub fn ahead_of(&self, commit: &impl Commit<T>) -> bool {
		self.head().ahead(commit)
	}

	#[inline]
	///
	pub fn behind_of(&self, commit: &impl Commit<T>) -> bool {
		self.head().behind(commit)
	}

	#[inline]
	///
	pub fn timestamp(&self) -> Timestamp {
		self.head().timestamp()
	}

	///
	pub fn head_owned(&self) -> CommitOwned<T> {
		self.head().into_owned()
	}
}

impl<T: Apply<Patch>, Patch> From<&Writer<T, Patch>> for Reader<T> {
	fn from(value: &Writer<T, Patch>) -> Self {
		value.reader()
	}
}