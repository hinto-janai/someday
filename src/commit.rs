//! Snapshots of data with timestamps

//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{Reader,Timestamp};
#[allow(unused_imports)] // docs
use crate::Writer;

//---------------------------------------------------------------------------------------------------- CommitOwned
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[derive(Copy,Clone,Debug,Hash,PartialEq,PartialOrd,Eq,Ord)]
/// Owned snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from the operations
/// like [`Writer::head()`] and [`Reader::head()`], but instead
/// of being shared like [`CommitRef`], it is fully owned.
///
/// ```rust
/// # use someday::*;
/// let (reader, _) = someday::new::<String>("hello".into());
///
/// // This is a ref-counted String.
/// let reference: CommitRef<String> = reader.head();
///
/// // This is an owned String.
/// let owned: CommitOwned<String> = reference.to_commit_owned();
///
/// // This may or may not actually deallocate the String.
/// drop(reference);
///
/// // The String's destructor will run here.
/// drop(owned);
/// ```
pub struct CommitOwned<T>
where
	T: Clone
{
	/// Timestamp of this [`Commit`].
	///
	/// Starts at 0, and increments by 1 every time a `commit`-like
	/// operation is called by the [`Writer`].
	pub timestamp: Timestamp,

	/// The generic data `T`.
	pub data: T,
}

//---------------------------------------------------------------------------------------------------- CommitOwned Trait
impl<T: Clone> TryFrom<CommitRef<T>> for CommitOwned<T> {
	type Error = CommitRef<T>;

	#[inline]
	/// This cheaply acquires ownership of a shared [`CommitRef`]
	/// if you are the only one holding onto it.
	fn try_from(commit: CommitRef<T>) -> Result<Self, Self::Error> {
		Arc::try_unwrap(commit)
	}
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for CommitOwned<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.data, f)
	}
}

//---------------------------------------------------------------------------------------------------- CommitRef
#[allow(clippy::module_name_repetitions)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
// #[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
// #[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
// #[derive(Clone,Debug,Hash,PartialEq,PartialOrd,Eq,Ord)]
/// Cheaply cloneable snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from operations
/// like [`Writer::head()`] and [`Reader::head()`].
///
/// It is shared data, and cheaply [`Clone`]-able.
///
/// This is just an alias for [`Arc<CommitOwned<T>>`].
///
/// [`Commit`] is implemented on this (`Arc<CommitOwned<T>>`).
pub type CommitRef<T> = Arc<CommitOwned<T>>;

//---------------------------------------------------------------------------------------------------- CommitRef Trait impl
impl<T: Clone> From<&Reader<T>> for CommitRef<T> {
	#[inline]
	fn from(reader: &Reader<T>) -> Self {
		reader.head()
	}
}

//---------------------------------------------------------------------------------------------------- Commit
#[allow(clippy::module_name_repetitions)]
/// Objects that act like a `Commit`
///
/// Notably:
/// 1. They store some data `T`
/// 2. They store a timestamp of that data `T`
///
/// [`CommitRef`] & [`CommitOwned`] both implement this.
///
/// This trait is sealed and cannot be implemented for types outside of `someday`.
pub trait Commit<T>
where
	Self: private::Sealed,
	T: Clone,
{
	/// The timestamp of this [`CommitRef`].
	///
	/// Starts at 0, and increments by 1 every time [`Writer::commit()`] is called.
	///
	/// This means this also represents how many
	/// `Patch`'s were applied to your data.
	///
	/// See [`Writer`] & [`Reader`] for more timestamp documentation.
	fn timestamp(&self) -> Timestamp;

	/// Acquire a reference to the shared inner data.
	fn data(&self) -> &T;

	/// Expensively clone the inner data, without consuming [`Self`]
	fn to_data(&self) -> T;

	/// Cheaply convert `Self` to the owned data `T` if possible
	///
	/// If this is a [`CommitRef`] and it is the only [`CommitRef::count`]
	/// reference, this call will acquire ownership for free.
	///
	/// If there are other instances of this [`CommitRef`], the
	/// internal data will be cloned directly.
	///
	/// This is free for [`CommitOwned`].
	fn into_data(self) -> T;

	/// Expensively clone [`Self`], without consuming [`Self`]
	fn to_commit_owned(&self) -> CommitOwned<T>;

	/// Cheaply convert `Self` to owned if possible
	///
	/// If this is a [`CommitRef`] and it is the only [`CommitRef::count`]
	/// reference, this call will acquire ownership for free.
	///
	/// If there are other instances of this [`CommitRef`], the
	/// internal data will be cloned directly.
	///
	/// This is a no-op for [`CommitOwned`].
	fn into_commit_owned(self) -> CommitOwned<T>;

	#[inline]
	/// If there is a difference in `self` and `other`'s timestamps or data.
	fn diff(&self, other: &impl Commit<T>) -> bool where T: PartialEq<T> {
		(self.diff_timestamp(other)) || (self.diff_data(other))
	}

	#[inline]
	/// If there is a difference in `self` & `other`'s timestamps.
	fn diff_timestamp(&self, other: &impl Commit<T>) -> bool {
		self.timestamp() != other.timestamp()
	}

	#[inline]
	/// If there is a difference in `self` & `other`'s timestamps.
	fn diff_data(&self, other: &impl Commit<T>) -> bool where T: PartialEq<T> {
		self.data() != other.data()
	}

	#[inline]
	/// If `self`'s timestamp is ahead of `other`'s timestamp.
	fn ahead(&self, other: &impl Commit<T>) -> bool {
		self.timestamp() > other.timestamp()
	}

	#[inline]
	/// If `self`'s timestamp is behind of `other`'s timestamp.
	fn behind(&self, other: &impl Commit<T>) -> bool {
		self.timestamp() < other.timestamp()
	}
}

/// Sealed trait module
mod private {
	/// Sealed trait, prevents non-pub(crate) impls
	pub trait Sealed {}
	impl<T: Clone> Sealed for crate::CommitOwned<T> {}
	impl<T: Clone> Sealed for crate::CommitRef<T> {}
}

impl<T> Commit<T> for CommitRef<T>
where
	T: Clone,
{
	#[inline]
	fn timestamp(&self) -> Timestamp {
		self.timestamp
	}
	#[inline]
	fn data(&self) -> &T {
		&self.data
	}

	#[inline]
	fn to_data(&self) -> T {
		self.data.clone()
	}

	#[inline]
	fn into_data(self) -> T {
		match Self::try_unwrap(self) {
			Ok(s) => s.data,
			Err(s) => s.data.clone(),
		}
	}

	#[inline]
	fn to_commit_owned(&self) -> CommitOwned<T> {
		CommitOwned {
			timestamp: self.timestamp,
			data: self.data.clone(),
		}
	}

	#[inline]
	fn into_commit_owned(self) -> CommitOwned<T> where T: Clone {
		match Self::try_unwrap(self) {
			Ok(s) => s,
			Err(s) => CommitOwned {
				timestamp: s.timestamp,
				data: s.data.clone(),
			}
		}
	}
}

impl<T> Commit<T> for CommitOwned<T>
where
	T: Clone,
{
	#[inline]
	fn timestamp(&self) -> Timestamp {
		self.timestamp
	}
	#[inline]
	fn data(&self) -> &T {
		&self.data
	}

	#[inline]
	fn to_data(&self) -> T {
		self.data.clone()
	}

	#[inline]
	fn into_data(self) -> T {
		self.data
	}

	#[inline]
	fn to_commit_owned(&self) -> Self {
		self.clone()
	}
	#[inline]
	fn into_commit_owned(self) -> Self {
		self
	}
}