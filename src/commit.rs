//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	Writer,Reader,Apply,Timestamp, patch::PatchBTreeMap,
};

//---------------------------------------------------------------------------------------------------- CommitOwned
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[derive(Copy,Clone,Debug,Hash)]
/// Owned snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`CommitRef`] of data received from the commit operations
/// like [`Writer::head()`] and [`Reader::head()`], but instead
/// of being shared like [`CommitRef`], it is fully owned.
///
/// ```rust
/// # use someday::{*,patch::*};
/// let (reader, _) = someday::new(String::from("hello"));
///
/// // This is a ref-counted String.
/// let CommitRef: CommitRef<String> = reader.head();
///
/// // This is an owned String.
/// let owned: CommitOwned<String> = CommitRef.into_owned();
/// // The String's destructor will run here.
/// drop(owned)
/// ```
pub struct CommitOwned<T> {
	/// Timestamp of this CommitRef.
	///
	/// Starts at 0, and increments by 1 every time [`Writer::apply()`] is called.
	///
	/// This means this also represents how many
	/// [`Operation`]'s were applied to your data.
	pub timestamp: Timestamp,

	/// The generic data `T`.
	pub data: T,
}

//---------------------------------------------------------------------------------------------------- CommitOwned Trait
impl<T> TryFrom<CommitRef<T>> for CommitOwned<T> {
	type Error = CommitRef<T>;

	/// This cheaply acquires ownership of a shared [`CommitRef`]
	/// if you are the only one holding onto it.
	fn try_from(commit: CommitRef<T>) -> Result<Self, Self::Error> {
		Arc::try_unwrap(commit.inner).map_err(|inner| CommitRef { inner })
	}
}

impl<T> std::ops::Deref for CommitOwned<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<T> AsRef<T> for CommitOwned<T> {
	fn as_ref(&self) -> &T {
		&self.data
	}
}

impl<T> std::borrow::Borrow<T> for CommitOwned<T> {
	fn borrow(&self) -> &T {
		&self.data
	}
}

impl<T: PartialEq> PartialEq for CommitOwned<T> {
	fn eq(&self, other: &Self) -> bool {
		self == other
	}
}

impl<T: PartialEq<T>> PartialEq<T> for CommitOwned<T> {
	fn eq(&self, other: &T) -> bool {
		self.data == *other
	}
}

impl<T: PartialEq<str>> PartialEq<str> for CommitOwned<T> {
	fn eq(&self, other: &str) -> bool {
		self.data == *other
	}
}

impl<T: PartialEq<[u8]>> PartialEq<[u8]> for CommitOwned<T> {
	fn eq(&self, other: &[u8]) -> bool {
		self.data == *other
	}
}

impl<T: PartialOrd<T>> PartialOrd<T> for CommitOwned<T> {
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.data.partial_cmp(&other)
	}
}

macro_rules! impl_traits {
	($target:ty => $($from:ty),* $(,)?) => {
		$(
			impl PartialEq<&$target> for CommitOwned<$from> {
				fn eq(&self, other: &&$target) -> bool {
					let s: &$target = &self.data;
					s == *other
				}
			}
			impl PartialOrd<&$target> for CommitOwned<$from> {
				fn partial_cmp(&self, other: &&$target) -> Option<std::cmp::Ordering> {
					let s: &$target = &self.data;
					s.partial_cmp(*other)
				}
			}

			impl AsRef<$target> for CommitOwned<$from> {
				fn as_ref(&self) -> &$target {
					self.data.as_ref()
				}
			}

			impl std::borrow::Borrow<$target> for CommitOwned<$from> {
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

impl<T: std::fmt::Display> std::fmt::Display for CommitOwned<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.data, f)
	}
}

//---------------------------------------------------------------------------------------------------- CommitRef
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[derive(Clone,Debug,Hash)]
/// Cheaply cloneable snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`CommitRef`] of data received from the commit operations
/// like [`Writer::head()`] and [`Reader::head()`].
///
/// It is shared data, and cheaply [`Clone`]-able (it is an [`Arc`] internally).
///
/// To get the inner data, use [`CommitRef::data()`].
///
/// [`CommitRef`] also implements convenience traits like [`Deref`] and [`PartialEq`] for your `T`:
/// ```rust
/// # use someday::{*,patch::*};
/// let (reader, _) = someday::new(String::from("hello"));
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
pub struct CommitRef<T> {
	pub(super) inner: Arc<CommitOwned<T>>,
}

impl<T> CommitRef<T> {
	#[inline]
	/// How many other shared instances of this CommitRef exist?
	///
	/// This is akin to [`Arc::strong_count`].
	///
	/// If this returns `1`, then calling [`CommitRef::into_owned`]
	/// and/or [`CommitOwned::try_into`] will be free.
	pub fn count(&self) -> usize {
		Arc::strong_count(&self.inner)
	}

	#[inline]
	///
	pub fn try_unwrap(self) -> Result<CommitOwned<T>, Self> {
		Arc::try_unwrap(self.inner).map_err(|inner| CommitRef { inner })
	}
}

//---------------------------------------------------------------------------------------------------- CommitRef Trait impl
impl<T> std::ops::Deref for CommitRef<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.inner.data
	}
}

impl<T> AsRef<T> for CommitRef<T> {
	fn as_ref(&self) -> &T {
		&self.inner.data
	}
}

impl<T> std::borrow::Borrow<T> for CommitRef<T> {
	fn borrow(&self) -> &T {
		&self.inner.data
	}
}

impl<T: PartialEq> PartialEq for CommitRef<T> {
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<T: PartialEq<T>> PartialEq<T> for CommitRef<T> {
	fn eq(&self, other: &T) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialEq<str>> PartialEq<str> for CommitRef<T> {
	fn eq(&self, other: &str) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialEq<[u8]>> PartialEq<[u8]> for CommitRef<T> {
	fn eq(&self, other: &[u8]) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialOrd<T>> PartialOrd<T> for CommitRef<T> {
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.inner.data.partial_cmp(&other)
	}
}


macro_rules! impl_traits {
	($target:ty => $($from:ty),* $(,)?) => {
		$(
			impl PartialEq<&$target> for CommitRef<$from> {
				fn eq(&self, other: &&$target) -> bool {
					let s: &$target = &self.inner.data;
					s == *other
				}
			}
			impl PartialOrd<&$target> for CommitRef<$from> {
				fn partial_cmp(&self, other: &&$target) -> Option<std::cmp::Ordering> {
					let s: &$target = &self.inner.data;
					s.partial_cmp(*other)
				}
			}

			impl AsRef<$target> for CommitRef<$from> {
				fn as_ref(&self) -> &$target {
					self.inner.data.as_ref()
				}
			}

			impl std::borrow::Borrow<$target> for CommitRef<$from> {
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

impl<T: std::fmt::Display> std::fmt::Display for CommitRef<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.inner.data, f)
	}
}

impl<T: Clone> From<&Reader<T>> for CommitRef<T> {
	fn from(reader: &Reader<T>) -> Self {
		reader.head()
	}
}

//---------------------------------------------------------------------------------------------------- Commit
/// Objects that act like a `Commit`
///
/// Notably:
/// 1. They store some data `T`
/// 2. They store a timestamp of that data `T`
///
/// [`CommitRef`] & [`CommitOwned`] both implement this.
///
/// This trait is sealed and cannot be implemented for types outside of `someday`.
pub trait Commit<T>: private::Sealed {
	/// The timestamp of this [`CommitRef`].
	///
	/// Starts at 0, and increments by 1 every time [`Writer::commit()`] is called.
	///
	/// This means this also represents how many
	/// [`Patch`](Apply)'s were applied to your data.
	///
	/// See [`Writer`] & [`Reader`] for more timestamp documentation.
	fn timestamp(&self) -> Timestamp;

	/// Acquire a reference to the shared inner data.
	fn data(&self) -> &T;

	/// Expensively clone the inner data, without consuming [`Self`]
	fn to_data(&self) -> T;

	/// Cheaply convert `Self` to the owned data `T` if possible
	///
	/// If this is a `CommitRef` and it is the only [`CommitRef::strong`]
	/// reference, this call will acquire ownership for free.
	///
	/// If there are other instances of this [`CommitRef`], the
	/// internal data will be cloned directly.
	///
	/// This is a no-op for `CommitOwned`.
	fn into_data(self) -> T;

	/// Expensively clone [`Self`], without consuming [`Self`]
	fn to_owned(&self) -> CommitOwned<T>;

	/// Cheaply convert `Self` to owned if possible
	///
	/// If this is a `CommitRef` and it is the only [`CommitRef::strong`]
	/// reference, this call will acquire ownership for free.
	///
	/// If there are other instances of this [`CommitRef`], the
	/// internal data will be cloned directly.
	///
	/// This is a no-op for `CommitOwned`.
	fn into_owned(self) -> CommitOwned<T>;

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
	fn eq(&self, other: &CommitRef<T>) -> bool {
		**self == **other.inner
	}
}

impl<T: Clone + PartialEq> PartialEq<&CommitRef<T>> for &CommitOwned<T> {
	fn eq(&self, other: &&CommitRef<T>) -> bool {
		**self == **other.inner
	}
}

impl<T: Clone + PartialEq> PartialEq<&CommitRef<T>> for CommitOwned<T> {
	fn eq(&self, other: &&CommitRef<T>) -> bool {
		**self == **other.inner
	}
}

mod private {
	pub trait Sealed {}
	impl<T> Sealed for crate::CommitOwned<T> {}
	impl<T> Sealed for crate::CommitRef<T> {}
}

impl<T> Commit<T> for CommitRef<T>
where
	T: Clone,
{
	#[inline(always)]
	fn timestamp(&self) -> Timestamp {
		self.inner.timestamp
	}
	#[inline(always)]
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
	fn to_owned(&self) -> CommitOwned<T> {
		CommitOwned {
			timestamp: self.inner.timestamp,
			data: self.inner.data.clone(),
		}
	}
	#[inline]
	fn into_owned(self) -> CommitOwned<T> where T: Clone {
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
	#[inline(always)]
	fn timestamp(&self) -> Timestamp {
		self.timestamp
	}
	#[inline(always)]
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

	#[inline(always)]
	fn to_owned(&self) -> CommitOwned<T> {
		self.clone()
	}
	#[inline(always)]
	fn into_owned(self) -> CommitOwned<T> {
		self
	}
}