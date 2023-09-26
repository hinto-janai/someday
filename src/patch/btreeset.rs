//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::BTreeSet;

//---------------------------------------------------------------------------------------------------- PatchBTreeSet
#[non_exhaustive]
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
/// Common operations for [`BTreeSet`]
pub enum PatchBTreeSet<T> {
	///
	Insert(T),
	///
	Remove(T),
	///
	Clear,
}


//---------------------------------------------------------------------------------------------------- Apply Impl
impl<T> Apply<PatchBTreeSet<T>> for BTreeSet<T>
where
	T: Clone + Ord,
{
	fn apply(
		patch: &PatchBTreeSet<T>,
		writer: &mut Self,
		reader: &Self,
	) {
		match patch {
			PatchBTreeSet::Insert(t) => { writer.insert(t.clone()); },
			PatchBTreeSet::Remove(t) => { writer.remove(t); },
			PatchBTreeSet::Clear     => writer.clear(),
		}
	}
}
