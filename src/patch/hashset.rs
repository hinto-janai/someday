//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::HashSet;

//---------------------------------------------------------------------------------------------------- PatchHashSet
#[non_exhaustive]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
/// Common operations for [`HashSet`]
pub enum PatchHashSet<T> {
	/// [`HashSet::insert`]
	Insert(T),
	/// [`HashSet::remove`]
	Remove(T),
	/// [`HashSet::clear`]
	Clear,
	/// [`HashSet::shrink_to_fit`]
	ShrinkToFit,
	/// [`HashSet::shrink_to`]
	ShrinkTo(usize),
	/// [`HashSet::reserve`]
	Reserve(usize),
}


//---------------------------------------------------------------------------------------------------- Apply Impl
impl<T> Apply<PatchHashSet<T>> for HashSet<T>
where
	T: Clone + std::cmp::Eq + PartialEq + std::hash::Hash,
{
	fn apply(
		patch: &mut PatchHashSet<T>,
		writer: &mut Self,
		_reader: &Self,
	) {
		match patch {
			PatchHashSet::Insert(t) => { writer.insert(t.clone()); },
			PatchHashSet::Remove(t)    => { writer.remove(t); },
			PatchHashSet::Clear        => writer.clear(),
			PatchHashSet::ShrinkToFit  => writer.shrink_to_fit(),
			PatchHashSet::ShrinkTo(u)  => writer.shrink_to(*u),
			PatchHashSet::Reserve(u)   => writer.reserve(*u),
		}
	}
}
