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
/// Common operations for [`HashSet`]
pub enum PatchHashSet<T> {
	///
	Insert(T),
	///
	Remove(T),
	///
	Clear,
	///
	ShrinkToFit,
	///
	ShrinkTo(usize),
	/// Reserves capacity for some number of additional elements in [`Values`]
	/// for the given key. If the given key does not exist, allocate an empty
	/// `Values` with the given capacity.
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
		reader: &Self,
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
