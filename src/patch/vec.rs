//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};

//---------------------------------------------------------------------------------------------------- OperationsVec
/// Common operations for [`Vec`]
///
/// See the [`README.md`](https://github.com/hinto-janai/someday) for example code using [`PatchVec`].
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub enum PatchVec<T> {
	/// [`Vec::push`]
	Push(T),
	/// [`Vec::clear`]
	Clear,
	/// [`Vec::insert`]
	Insert(T, usize),
	/// [`Vec::remove`]
	Remove(usize),
	/// [`Vec::reserve`]
	Reserve(usize),
	/// [`Vec::reserve_exact`]
	ReserveExact(usize),
	/// [`Vec::shrink_to`]
	ShrinkTo(usize),
	/// [`Vec::shrink_to_fit`]
	ShrinkToFit,
	/// [`Vec::truncate`]
	Truncate(usize),
}

impl<T: Clone> Apply<PatchVec<T>> for Vec<T> {
	fn apply(
		operation: &mut PatchVec<T>,
		writer: &mut Self,
		_reader: &Self,
	) {
		match operation {
			PatchVec::Push(t)         => writer.push(t.clone()),
			PatchVec::Clear           => writer.clear(),
			PatchVec::Insert(t, i)    => writer.insert(*i, t.clone()),
			PatchVec::Remove(i)       => { writer.remove(*i); },
			PatchVec::Reserve(i)      => writer.reserve(*i),
			PatchVec::ReserveExact(i) => writer.reserve_exact(*i),
			PatchVec::ShrinkTo(u)     => writer.shrink_to(*u),
			PatchVec::ShrinkToFit     => writer.shrink_to_fit(),
			PatchVec::Truncate(u)     => writer.truncate(*u),
		}
	}
}