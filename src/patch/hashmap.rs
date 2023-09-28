//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::HashMap;

//---------------------------------------------------------------------------------------------------- PatchHashMap
#[non_exhaustive]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
/// Common operations for [`HashMap`]
pub enum PatchHashMap<K, V> {
	/// [`HashMap::insert`]
	Insert(K, V),
	/// [`HashMap::remove`]
	Remove(K),
	/// [`HashMap::clear`]
	Clear,
	/// [`HashMap::shrink_to_fit`]
	ShrinkToFit,
	/// [`HashMap::shrink_to`]
	ShrinkTo(usize),
	/// [`HashMap::reserve`]
	Reserve(usize),
}

//---------------------------------------------------------------------------------------------------- Apply Impl
impl<K, V> Apply<PatchHashMap<K, V>> for HashMap<K, V>
where
	K: Clone + std::cmp::Eq + PartialEq + std::hash::Hash,
	V: Clone,
{
	fn apply(
		patch: &mut PatchHashMap<K, V>,
		writer: &mut Self,
		_reader: &Self,
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
