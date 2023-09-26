//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::HashMap;

//---------------------------------------------------------------------------------------------------- PatchHashMap
#[non_exhaustive]
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
/// Common operations for [`HashMap`]
pub enum PatchHashMap<K, V> {
	///
	Insert(K, V),
	///
	Remove(K),
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
impl<K, V> Apply<PatchHashMap<K, V>> for HashMap<K, V>
where
	K: Clone + std::cmp::Eq + PartialEq + std::hash::Hash,
	V: Clone,
{
	fn apply(
		patch: &PatchHashMap<K, V>,
		writer: &mut Self,
		reader: &Self,
	) {
		match patch {
			PatchHashMap::Insert(k, v) => { writer.insert(k.clone(), v.clone()); },
			PatchHashMap::Remove(k)    => { writer.remove(k); },
			PatchHashMap::Clear        => writer.clear(),
			PatchHashMap::ShrinkToFit  => writer.shrink_to_fit(),
			PatchHashMap::ShrinkTo(u)  => writer.shrink_to(*u),
			PatchHashMap::Reserve(u)   => writer.reserve(*u),
		}
	}
}
