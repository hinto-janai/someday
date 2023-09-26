//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::BTreeSet;

//---------------------------------------------------------------------------------------------------- PatchBTreeSet
#[non_exhaustive]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
/// Common operations for [`BTreeSet`]
pub enum PatchBTreeSet<T> {
	/// [`BTreeSet::insert`]
	Insert(T),
	/// [`BTreeSet::remove`]
	Remove(T),
	/// [`BTreeSet::clear`]
	Clear,
}


//---------------------------------------------------------------------------------------------------- Apply Impl
impl<T> Apply<PatchBTreeSet<T>> for BTreeSet<T>
where
	T: Clone + Ord,
{
	fn apply(
		patch: &mut PatchBTreeSet<T>,
		writer: &mut Self,
		_reader: &Self,
	) {
		match patch {
			PatchBTreeSet::Insert(t) => { writer.insert(t.clone()); },
			PatchBTreeSet::Remove(t) => { writer.remove(t); },
			PatchBTreeSet::Clear     => writer.clear(),
		}
	}
}
