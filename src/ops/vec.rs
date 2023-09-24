//---------------------------------------------------------------------------------------------------- use
use crate::{
	Operation,
	Writer,
	Reader,
};

//---------------------------------------------------------------------------------------------------- OperationsVec
/// Common operations for [`Vec`]
///
/// These are very basic, common operations that can be done to a [`Vec`].
/// Anything with trait bounds or complicated returned values is not enumerated.
///
/// This implementations [`Operation`] for [`Vec`], so that it can be used for [`Writer`] & [`Reader`].
///
/// ```rust
/// # use someday::*;
/// # use someday::ops::*;
/// // Create vector.
/// let v = vec!["a"];
/// let c = v.clone();
///
/// // Our target.
/// let target = vec!["a", "b"];
///
/// // Create Writer/Reader.
/// let (mut w, r) = someday::new(v);
///
/// // The readers see the data.
/// assert_eq!(r.snapshot(), c);
/// assert_eq!(r.snapshot().timestamp(), 0);
///
/// // Writer writes some data, but does not commit.
/// w.apply(OperationVec::Push("b"));
/// // Current writer data is updated.
/// assert_eq!(*w.read(), target);
/// // But readers still see old data.
/// assert_eq!(r.snapshot(), c);
///
/// // Commit writes.
/// w.commit();
///
/// // Now readers see updates.
/// assert_eq!(r.snapshot(), target);
/// assert_eq!(r.snapshot().timestamp(), 1);
/// ```
pub enum OperationVec<T> {
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
	Resize(usize, T),
	///
	ShrinkTo(usize),
	///
	ShrinkToFit,
	///
	Truncate(usize),
}

impl<T> crate::ops::Operation<OperationVec<T>> for Vec<T>
where
	T: Clone,
{
	fn apply(&mut self, operation: &mut OperationVec<T>) {
		match operation {
			OperationVec::Push(t)         => self.push(t.clone()),
			OperationVec::Clear           => self.clear(),
			OperationVec::Insert(t, i)    => self.insert(*i, t.clone()),
			OperationVec::Remove(i)       => { self.remove(*i); },
			OperationVec::Reserve(i)      => self.reserve(*i),
			OperationVec::ReserveExact(i) => self.reserve_exact(*i),
			OperationVec::Resize(i, t)    => self.resize(*i, t.clone()),
			OperationVec::ShrinkTo(u)     => self.shrink_to(*u),
			OperationVec::ShrinkToFit     => self.shrink_to_fit(),
			OperationVec::Truncate(u)     => self.truncate(*u),
		}
	}
}