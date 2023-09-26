//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::BTreeMap;

//---------------------------------------------------------------------------------------------------- PatchBTreeMap
#[non_exhaustive]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
/// Common operations for [`BTreeMap`]
pub enum PatchBTreeMap<K, V> {
	/// [`BTreeMap::insert`]
	Insert(K, V),
	/// [`BTreeMap::remove`]
	Remove(K),
	/// [`BTreeMap::clear`]
	Clear,
}


//---------------------------------------------------------------------------------------------------- Apply Impl
impl<K, V> Apply<PatchBTreeMap<K, V>> for BTreeMap<K, V>
where
	K: Clone + Ord,
	V: Clone,
{
	fn apply(
		patch: &mut PatchBTreeMap<K, V>,
		writer: &mut Self,
		_reader: &Self,
	) {
		match patch {
			PatchBTreeMap::Insert(k, v) => { writer.insert(k.clone(), v.clone()); },
			PatchBTreeMap::Remove(k)    => { writer.remove(k); },
			PatchBTreeMap::Clear        => writer.clear(),
		}
	}
}
