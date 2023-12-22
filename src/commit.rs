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
		Arc::try_unwrap(commit.inner).map_err(|inner| CommitRef { inner })
	}
}

impl<T: Clone> std::ops::Deref for CommitOwned<T> {
	type Target = T;
	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<T: Clone> AsRef<T> for CommitOwned<T> {
	#[inline]
	fn as_ref(&self) -> &T {
		&self.data
	}
}

impl<T: Clone> std::borrow::Borrow<T> for CommitOwned<T> {
	#[inline]
	fn borrow(&self) -> &T {
		&self.data
	}
}

impl<T: Clone + PartialEq<T>> PartialEq<T> for CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &T) -> bool {
		self.data == *other
	}
}

impl<T: Clone + PartialEq<str>> PartialEq<str> for CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &str) -> bool {
		self.data == *other
	}
}

impl<T: Clone + PartialEq<[u8]>> PartialEq<[u8]> for CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &[u8]) -> bool {
		self.data == *other
	}
}

impl<T: Clone + PartialOrd<T>> PartialOrd<T> for CommitOwned<T> {
	#[inline]
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.data.partial_cmp(other)
	}
}

/// Implement traits on `CommitOwned`
macro_rules! impl_traits {
	($target:ty => $($from:ty),* $(,)?) => {
		$(
			impl PartialEq<&$target> for CommitOwned<$from> {
				#[inline]
				fn eq(&self, other: &&$target) -> bool {
					let s: &$target = &self.data;
					s == *other
				}
			}

			impl PartialOrd<&$target> for CommitOwned<$from> {
				#[inline]
				fn partial_cmp(&self, other: &&$target) -> Option<std::cmp::Ordering> {
					let s: &$target = &self.data;
					s.partial_cmp(*other)
				}
			}

			impl AsRef<$target> for CommitOwned<$from> {
				#[inline]
				fn as_ref(&self) -> &$target {
					self.data.as_ref()
				}
			}

			impl std::borrow::Borrow<$target> for CommitOwned<$from> {
				#[inline]
				fn borrow(&self) -> &$target {
					self.data.as_ref()
				}
			}
		)*
	};
}
impl_traits! { str =>
	String,
	Box<str>,
	Arc<str>,
	std::rc::Rc<str>,
}
impl_traits! { [u8] =>
	Box<[u8]>,
	Arc<[u8]>,
	std::rc::Rc<[u8]>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for CommitOwned<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.data, f)
	}
}

//---------------------------------------------------------------------------------------------------- CommitRef
#[allow(clippy::module_name_repetitions)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[derive(Clone,Debug,Hash,PartialEq,PartialOrd,Eq,Ord)]
/// Cheaply cloneable snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from operations
/// like [`Writer::head()`] and [`Reader::head()`].
///
/// It is shared data, and cheaply [`Clone`]-able (it is an [`Arc`] internally).
///
/// To get the inner data, use [`CommitRef::data()`].
///
/// [`CommitRef`] also implements convenience traits like [`PartialEq`] for your `T`:
/// ```rust
/// # use someday::*;
/// let (reader, _) = someday::new::<String>("hello".into());
///
/// let commit: CommitRef<String> = reader.head();
///
/// // `PartialEq` the CommitRef directly with a string
/// assert_eq!(commit, "hello");
///
/// // String implements `Display`, so CommitRef also
/// // implements `Display` and can call `.to_string()`.
/// let string = commit.to_string();
/// assert_eq!(string, "hello");
/// ```
pub struct CommitRef<T>
where
	T: Clone
{
	/// Shared pointer to a `CommitOwned<T>.
	pub(super) inner: Arc<CommitOwned<T>>,
}

impl<T> CommitRef<T>
where
	T: Clone
{
	#[inline]
	#[must_use]
	/// How many other shared instances of this [`CommitRef`] exist?
	///
	/// This is akin to [`Arc::strong_count`].
	pub fn count(&self) -> usize {
		Arc::strong_count(&self.inner)
	}

	#[inline]
	/// Cheaply convert to an [`CommitOwned`] if possible
	///
	/// This is akin to [`Arc::try_unwrap`].
	///
	/// # Errors
	/// This attempts to take ownership of the backing data
	/// inside this [`CommitRef`]. If there are other references
	/// ([`CommitRef::count()`]) then this function will fail
	/// and return the `self` input back.
	///
	/// If there are no other references, this will cheaply
	/// acquire ownership of the `T` data.
	pub fn try_unwrap(self) -> Result<CommitOwned<T>, Self> {
		Arc::try_unwrap(self.inner).map_err(|inner| Self { inner })
	}
}

//---------------------------------------------------------------------------------------------------- CommitRef Trait impl
impl<T: Clone> std::ops::Deref for CommitRef<T> {
	type Target = T;
	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.inner.data
	}
}

impl<T: Clone> AsRef<T> for CommitRef<T> {
	#[inline]
	fn as_ref(&self) -> &T {
		&self.inner.data
	}
}

impl<T: Clone> std::borrow::Borrow<T> for CommitRef<T> {
	#[inline]
	fn borrow(&self) -> &T {
		&self.inner.data
	}
}

impl<T: Clone + PartialEq<T>> PartialEq<T> for CommitRef<T> {
	#[inline]
	fn eq(&self, other: &T) -> bool {
		self.inner.data == *other
	}
}

impl<T: Clone + PartialEq<str>> PartialEq<str> for CommitRef<T> {
	#[inline]
	fn eq(&self, other: &str) -> bool {
		self.inner.data == *other
	}
}

impl<T: Clone + PartialEq<[u8]>> PartialEq<[u8]> for CommitRef<T> {
	#[inline]
	fn eq(&self, other: &[u8]) -> bool {
		self.inner.data == *other
	}
}

impl<T: Clone + PartialOrd<T>> PartialOrd<T> for CommitRef<T> {
	#[inline]
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.inner.data.partial_cmp(other)
	}
}

/// Implement traits on `CommitRef`.
macro_rules! impl_traits {
	($target:ty => $($from:ty),* $(,)?) => {
		$(
			impl PartialEq<&$target> for CommitRef<$from> {
				#[inline]
				fn eq(&self, other: &&$target) -> bool {
					let s: &$target = &self.inner.data;
					s == *other
				}
			}
			impl PartialOrd<&$target> for CommitRef<$from> {
				#[inline]
				fn partial_cmp(&self, other: &&$target) -> Option<std::cmp::Ordering> {
					let s: &$target = &self.inner.data;
					s.partial_cmp(*other)
				}
			}

			impl AsRef<$target> for CommitRef<$from> {
				#[inline]
				fn as_ref(&self) -> &$target {
					self.inner.data.as_ref()
				}
			}

			impl std::borrow::Borrow<$target> for CommitRef<$from> {
				#[inline]
				fn borrow(&self) -> &$target {
					self.inner.data.as_ref()
				}
			}
		)*
	};
}
impl_traits! { str =>
	String,
	Box<str>,
	Arc<str>,
	std::rc::Rc<str>,
}
impl_traits! { [u8] =>
	Box<[u8]>,
	Arc<[u8]>,
	std::rc::Rc<[u8]>,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for CommitRef<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.inner.data, f)
	}
}

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
	/// If there is a difference in `self` & `other`'s timestamps.
	fn diff(&self, other: &impl Commit<T>) -> bool where T: PartialEq<T> {
		(self.diff_timestamp(other)) && (self.diff_data(other))
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

impl<T: Clone + PartialEq> PartialEq<CommitRef<T>> for &CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &CommitRef<T>) -> bool {
		**self == **other.inner
	}
}

impl<T: Clone + PartialEq> PartialEq<&CommitRef<T>> for &CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &&CommitRef<T>) -> bool {
		**self == **other.inner
	}
}

impl<T: Clone + PartialEq> PartialEq<&CommitRef<T>> for CommitOwned<T> {
	#[inline]
	fn eq(&self, other: &&CommitRef<T>) -> bool {
		**self == **other.inner
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
		self.inner.timestamp
	}
	#[inline]
	fn data(&self) -> &T {
		&self.inner.data
	}

	#[inline]
	fn to_data(&self) -> T {
		self.inner.data.clone()
	}

	#[inline]
	fn into_data(self) -> T {
		match Arc::try_unwrap(self.inner) {
			Ok(s) => s.data,
			Err(s) => s.data.clone(),
		}
	}

	#[inline]
	fn to_commit_owned(&self) -> CommitOwned<T> {
		CommitOwned {
			timestamp: self.inner.timestamp,
			data: self.inner.data.clone(),
		}
	}

	#[inline]
	fn into_commit_owned(self) -> CommitOwned<T> where T: Clone {
		match Arc::try_unwrap(self.inner) {
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