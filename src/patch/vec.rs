//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	ApplyReturnLt,
	Writer,
	Reader,
};
use std::vec::Drain;
use std::ops::{Bound,RangeBounds,Range};

//---------------------------------------------------------------------------------------------------- PatchVec
/// Common operations for [`Vec`]
///
/// See the [`README.md`](https://github.com/hinto-janai/someday) for example code using [`PatchVec`].
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum PatchVec<T> {
	/// [`Vec::push`]
	Push(T),
	/// [`Vec::clear`]
	Clear,
	/// [`Vec::insert`]
	Insert(T, usize),
	/// [`Vec::remove`]
	Remove(usize),
	/// [`Vec::drain`]
	Drain(Range<usize>),
	/// [`Vec::drain`] on all elements `..`
	DrainAll,
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
			PatchVec::Drain(r)        => { writer.drain(r.clone()); },
			PatchVec::DrainAll        => { writer.drain(..); },
			PatchVec::Reserve(i)      => writer.reserve(*i),
			PatchVec::ReserveExact(i) => writer.reserve_exact(*i),
			PatchVec::ShrinkTo(u)     => writer.shrink_to(*u),
			PatchVec::ShrinkToFit     => writer.shrink_to_fit(),
			PatchVec::Truncate(u)     => writer.truncate(*u),
		}
	}
}

//---------------------------------------------------------------------------------------------------- PatchVecDrain
#[derive(Clone)]
/// Specialized patch for [`PatchVec`] implementing [`Vec::drain()`]
///
/// ## Usage
/// ```rust
/// # use someday::*;
/// # use std::vec::Drain;
/// // Create Vec.
/// let vec = vec![0, 1, 2];
///
/// // Create Reader/Writer.
/// let (r, mut w) = someday::new(vec);
///
/// // Drain it.
/// let drain: Drain<'_, usize> = w.commit_return_lt(
/// 	PatchVecDrain(0..w.data().len())
/// );
///
/// let collected: Vec<usize> = drain.collect();
/// assert_eq!(collected, vec![0, 1, 2]);
/// ```
pub struct PatchVecDrain(pub Range<usize>);
impl<T> From<PatchVecDrain> for PatchVec<T> {
	fn from(value: PatchVecDrain) -> Self {
		PatchVec::Drain(value.0)
	}
}

impl<'a, T> ApplyReturnLt<'a, PatchVec<T>, PatchVecDrain, Drain<'a, T>> for Vec<T>
where
	T: Clone,
{
	fn apply_return_lt(
		patch: &mut PatchVecDrain,
		writer: &'a mut Self,
		_reader: &Self,
	) -> Drain<'a, T> {
		writer.drain(patch.0.clone())
	}
}

//---------------------------------------------------------------------------------------------------- PatchVecDrainAll
#[derive(Clone)]
/// Specialized patch for [`PatchVec`] implementing [`Vec::drain()`] on all elements `..`
///
/// ## Usage
/// ```rust
/// # use someday::*;
/// # use std::vec::Drain;
/// // Create Vec.
/// let vec = vec![0, 1, 2];
///
/// // Create Reader/Writer.
/// let (r, mut w) = someday::new(vec);
///
/// // Drain it.
/// let drain: Drain<'_, usize> = w.commit_return_lt(PatchVecDrainAll);
///
/// let collected: Vec<usize> = drain.collect();
/// assert_eq!(collected, vec![0, 1, 2]);
/// ```
pub struct PatchVecDrainAll;
impl<T> From<PatchVecDrainAll> for PatchVec<T> {
	fn from(_value: PatchVecDrainAll) -> Self {
		PatchVec::DrainAll
	}
}

impl<'a, T> ApplyReturnLt<'a, PatchVec<T>, PatchVecDrainAll, Drain<'a, T>> for Vec<T>
where
	T: Clone,
{
	fn apply_return_lt(
		_patch: &mut PatchVecDrainAll,
		writer: &'a mut Self,
		_reader: &Self,
	) -> Drain<'a, T> {
		writer.drain(..)
	}
}