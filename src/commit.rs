//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	Writer,Reader,Timestamp,
};

//---------------------------------------------------------------------------------------------------- CommitOwned
#[derive(Clone)]
/// Owned container of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from the commit operations
/// like [`Writer::commit()`] and [`Reader::commit()`], but instead
/// of being shared like [`Commit`], it is fully owned.
///
/// ```rust
/// # use someday::{*,patch::*};
/// let (reader, _) = someday::new(String::from("hello"));
///
/// // This is a ref-counted String.
/// let Commit: Commit<String> = reader.head();
///
/// // This is an owned String.
/// let owned: CommitOwned<String> = Commit.into_owned();
/// // The String's destructor will run here.
/// drop(owned)
/// ```
pub struct CommitOwned<T> {
	/// Timestamp of this Commit.
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
impl<T> TryFrom<Commit<T>> for CommitOwned<T> {
	type Error = Commit<T>;

	/// This cheaply acquires ownership of a shared [`Commit`]
	/// if you are the only one holding onto it.
	fn try_from(commit: Commit<T>) -> Result<Self, Self::Error> {
		Arc::try_unwrap(commit.inner).map_err(|inner| Commit { inner })
	}
}

impl<T: PartialEq> PartialEq for CommitOwned<T> {
	fn eq(&self, other: &Self) -> bool {
		self == other
	}
}

impl<T> std::fmt::Display for CommitOwned<T>
where
	T: std::fmt::Display,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.data, f)
	}
}

impl<T> std::fmt::Debug for CommitOwned<T>
where
	T: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CommitOwned")
			.field("timestamp", &self.timestamp)
			.field("data", &self.data)
			.finish()
	}
}

//---------------------------------------------------------------------------------------------------- Commit
#[derive(Clone)]
/// Cheaply cloneable Commit of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from the commit operations
/// like [`Writer::commit()`] and [`Reader::commit()`].
///
/// It is shared data, and cheaply [`Clone`]-able (it is an [`Arc`] internally).
///
/// To get the inner data, use [`Commit::data()`].
///
/// [`Commit`] also implements convenience traits like [`Deref`] and [`PartialEq`] for your `T`:
/// ```rust
/// # use someday::{*,patch::*};
/// let (reader, _) = someday::new(String::from("hello"));
///
/// let Commit: Commit<String> = reader.head();
///
/// // `PartialEq` the Commit directly with a string
/// assert_eq!(Commit, "hello");
///
/// // String implements `Display`, so Commit also
/// // implements `Display` and can call `.to_string()`.
/// let string = Commit.to_string();
/// assert_eq!(string, "hello");
/// ```
pub struct Commit<T> {
	pub(super) inner: Arc<CommitOwned<T>>,
}

impl<T> Commit<T> {
	#[inline]
	/// The timestamp of this [`Commit`].
	///
	/// Starts at 0, and increments by 1 every time [`Writer::apply()`] is called.
	///
	/// This means this also represents how many
	/// [`Operation`]'s were applied to your data.
	///
	/// See [`Writer`] & [`Reader`] for more timestamp documentation.
	pub fn timestamp(&self) -> Timestamp {
		self.inner.timestamp
	}

	#[inline]
	/// Acquire a reference to the shared inner data.
	pub fn data(&self) -> &T {
		&self.inner.data
	}

	/// How many other shared instances of this Commit exist?
	///
	/// This is akin to [`Arc::strong_count`].
	///
	/// If this returns `1`, then calling [`Commit::into_owned`]
	/// and/or [`CommitOwned::try_into`] will be free.
	pub fn count(&self) -> usize {
		Arc::strong_count(&self.inner)
	}

	/// Expensively clone the inner data, without consuming [`Self`]
	///
	/// This will not create a cheap clone, it will acquire
	/// a [`CommitOwned`] by cloning the inner data directly.
	pub fn to_owned(&self) -> CommitOwned<T> where T: Clone {
		CommitOwned {
			timestamp: self.inner.timestamp,
			data: self.inner.data.clone(),
		}
	}

	/// Consume [`Self`] and acquire the inner data
	///
	/// If this is the only [`Commit::strong`] reference to
	/// this Commit, this call will acquire ownership for free.
	///
	/// If there are other instances of this Commit, the
	/// internal data will be cloned directly.
	pub fn into_owned(self) -> CommitOwned<T> where T: Clone {
		match Arc::try_unwrap(self.inner) {
			Ok(s) => s,
			Err(s) => CommitOwned {
				timestamp: s.timestamp,
				data: s.data.clone(),
			}
		}
	}
}

//---------------------------------------------------------------------------------------------------- Commit Trait impl
impl<T> std::ops::Deref for Commit<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.inner.data
	}
}

impl<T> AsRef<T> for Commit<T> {
	fn as_ref(&self) -> &T {
		&self.inner.data
	}
}

impl<T> std::borrow::Borrow<T> for Commit<T> {
	fn borrow(&self) -> &T {
		&self.inner.data
	}
}

impl<T: PartialEq> PartialEq for Commit<T> {
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<T: PartialEq<T>> PartialEq<T> for Commit<T> {
	fn eq(&self, other: &T) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialEq<str>> PartialEq<str> for Commit<T> {
	fn eq(&self, other: &str) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialEq<[u8]>> PartialEq<[u8]> for Commit<T> {
	fn eq(&self, other: &[u8]) -> bool {
		self.inner.data == *other
	}
}

impl<T: PartialOrd<T>> PartialOrd<T> for Commit<T> {
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.inner.data.partial_cmp(&other)
	}
}

macro_rules! impl_traits {
	($target:ty => $($from:ty),* $(,)?) => {
		$(
			impl PartialEq<&$target> for Commit<$from> {
				fn eq(&self, other: &&$target) -> bool {
					let s: &$target = &self.inner.data;
					s == *other
				}
			}
			impl PartialOrd<&$target> for Commit<$from> {
				fn partial_cmp(&self, other: &&$target) -> Option<std::cmp::Ordering> {
					let s: &$target = &self.inner.data;
					s.partial_cmp(*other)
				}
			}

			impl AsRef<$target> for Commit<$from> {
				fn as_ref(&self) -> &$target {
					self.inner.data.as_ref()
				}
			}

			impl std::borrow::Borrow<$target> for Commit<$from> {
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

impl<T: std::fmt::Display> std::fmt::Display for Commit<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.inner.data, f)
	}
}

impl<T: std::fmt::Debug> std::fmt::Debug for Commit<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Commit")
			.field("timestamp", &self.inner.timestamp)
			.field("data", &self.inner.data)
			.finish()
	}
}