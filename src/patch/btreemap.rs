//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::collections::BTreeMap;

//---------------------------------------------------------------------------------------------------- PatchBTreeMap
#[non_exhaustive]
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
/// Common operations for [`BTreeMap`]
pub enum PatchBTreeMap<K, V> {
	///
	Insert(K, V),
	///
	Remove(K),
	///
	Clear,
}


//---------------------------------------------------------------------------------------------------- Apply Impl
impl<K, V> Apply<PatchBTreeMap<K, V>> for BTreeMap<K, V>
where
	K: Clone + Ord,
	V: Clone,
{
	fn apply(
		patch: &PatchBTreeMap<K, V>,
		writer: &mut Self,
		reader: &Self,
	) {
		match patch {
			PatchBTreeMap::Insert(k, v) => { writer.insert(k.clone(), v.clone()); },
			PatchBTreeMap::Remove(k)    => { writer.remove(k); },
			PatchBTreeMap::Clear        => writer.clear(),
		}
	}
}
