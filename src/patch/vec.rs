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
#[derive(PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub enum PatchVec<T> {
	///
	Push(T),
	///
	Clear,
	///
	Insert(T, usize),
	///
	Remove(usize),
	///
	Reserve(usize),
	///
	ReserveExact(usize),
	///
	ShrinkTo(usize),
	///
	ShrinkToFit,
	///
	Truncate(usize),
}

impl<T: Clone> Apply<PatchVec<T>> for Vec<T> {
	fn apply(
		operation: &PatchVec<T>,
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