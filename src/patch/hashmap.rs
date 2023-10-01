//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	ApplyReturn,
	Writer,
	Reader,
};
use std::collections::hash_map::{
	HashMap,
	Entry,
};

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
	/// [`HashMap::entry`]
	Entry(K),
	/// [`HashMap::clear`]
	Clear,
	/// [`HashMap::shrink_to_fit`]
	ShrinkToFit,
	/// [`HashMap::shrink_to`]
	ShrinkTo(usize),
	/// [`HashMap::reserve`]
	Reserve(usize),
}

//---------------------------------------------------------------------------------------------------- ApplyReturn Impl
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
			// Return values.
			PatchHashMap::Insert(k, v) => { writer.insert(k.clone(), v.clone()); },
			PatchHashMap::Remove(k)    => { writer.remove(k);  },

			// No-op.
			PatchHashMap::Entry(_)     => (),

			PatchHashMap::Clear        => writer.clear(),
			PatchHashMap::ShrinkToFit  => writer.shrink_to_fit(),
			PatchHashMap::ShrinkTo(u)  => writer.shrink_to(*u),
			PatchHashMap::Reserve(u)   => writer.reserve(*u),
		}
	}
}

//---------------------------------------------------------------------------------------------------- PatchHashMapInsert
#[derive(Clone)]
/// Specialized patch for [`PatchHashMap`] implementing [`HashMap::insert()`]
///
/// ## Usage
/// ```rust
/// # use someday::*;
/// # use std::collections::hash_map::*;
/// // Create HashMap
/// let h = HashMap::from([(0, "old".into())]);
///
/// // Create Reader/Writer.
/// let (r, mut w) = someday::new(h);
///
/// // Insert a value that already exists
/// // and take ownership of the old entry.
/// let old: Option<String> = w.commit_return(PatchHashMapInsert {
/// 	key: 0,
/// 	value: "new".into(),
/// });
///
/// assert_eq!(old.unwrap(), "old");
/// ```
pub struct PatchHashMapInsert<K, V> {
	///
	pub key: K,
	///
	pub value: V,
}
impl<K, V> From<PatchHashMapInsert<K, V>> for PatchHashMap<K, V> {
	fn from(value: PatchHashMapInsert<K, V>) -> Self { PatchHashMap::Insert(value.key, value.value) }
}
impl<K, V> ApplyReturn<PatchHashMap<K, V>, PatchHashMapInsert<K, V>, Option<V>> for HashMap<K, V>
where
	K: Clone + std::cmp::Eq + PartialEq + std::hash::Hash,
	V: Clone,
{
	fn apply_return(
		patch: &mut PatchHashMapInsert<K, V>,
		writer: &mut Self,
		_reader: &Self,
	) -> Option<V> {
		writer.insert(patch.key.clone(), patch.value.clone())
	}
}

//---------------------------------------------------------------------------------------------------- PatchHashMapRemove
#[derive(Clone)]
/// Specialized patch for [`PatchHashMap`] implementing [`HashMap::remove()`]
///
/// ## Usage
/// ```rust
/// # use someday::*;
/// # use std::collections::hash_map::*;
/// // Create HashMap
/// let h = HashMap::from([(0, "string".into())]);
///
/// // Create Reader/Writer.
/// let (r, mut w) = someday::new(h);
///
/// // Remove a value and take ownership.
/// let removed: Option<String> = w.commit_return(PatchHashMapRemove(0));
///
/// assert_eq!(removed.unwrap(), "string");
/// ```
pub struct PatchHashMapRemove<K>(pub K);
impl<K, V> From<PatchHashMapRemove<K>> for PatchHashMap<K, V> {
	fn from(value: PatchHashMapRemove<K>) -> Self { PatchHashMap::Remove(value.0) }
}
impl<K, V> ApplyReturn<PatchHashMap<K, V>, PatchHashMapRemove<K>, Option<V>> for HashMap<K, V>
where
	K: Clone + std::cmp::Eq + PartialEq + std::hash::Hash,
	V: Clone,
{
	fn apply_return(
		patch: &mut PatchHashMapRemove<K>,
		writer: &mut Self,
		_reader: &Self,
	) -> Option<V> {
		writer.remove(&patch.0)
	}
}