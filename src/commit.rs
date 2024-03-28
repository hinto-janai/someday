//! Snapshots of data with timestamps.

//---------------------------------------------------------------------------------------------------- Use
#[allow(unused_imports)] // docs
use crate::Writer;
use crate::{Reader, Timestamp};
use std::sync::Arc;

//---------------------------------------------------------------------------------------------------- Commit
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Copy, Clone, Debug, Hash, PartialEq, PartialOrd, Eq, Ord)]
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
/// let owned: Commit<String> = reference.as_ref().clone();
///
/// // This may or may not actually deallocate the String.
/// drop(reference);
///
/// // The String's destructor will run here.
/// drop(owned);
/// ```
pub struct Commit<T: Clone> {
    /// Timestamp of this [`Commit`].
    ///
    /// Starts at 0, and increments by 1 every time a `commit`-like
    /// operation is called by the [`Writer`].
    pub timestamp: Timestamp,

    /// The generic data `T`.
    pub data: T,
}

//---------------------------------------------------------------------------------------------------- Commit Impl
impl<T: Clone> Commit<T> {
    #[inline]
    /// If there is a difference in `self` and `other`'s timestamps or data.
    ///
    /// ```rust
    /// # use someday::*;
    /// // Timestamp is different.
    /// let commit_1 = Commit { timestamp: 0, data: "a" };
    /// let commit_2 = Commit { timestamp: 1, data: "a" };
    /// assert!(commit_1.diff(&commit_2));
    ///
    /// // Data is different.
    /// let commit_3 = Commit { timestamp: 0, data: "a" };
    /// let commit_4 = Commit { timestamp: 0, data: "b" };
    /// assert!(commit_3.diff(&commit_4));
    ///
    /// // Same.
    /// let commit_5 = Commit { timestamp: 0, data: "a" };
    /// let commit_6 = Commit { timestamp: 0, data: "a" };
    /// assert!(!commit_5.diff(&commit_6));
    /// ```
    pub fn diff(&self, other: &Self) -> bool
    where
        T: PartialEq<T>,
    {
        (self.diff_timestamp(other)) || (self.diff_data(other))
    }

    #[inline]
    /// If there is a difference in `self` & `other`'s timestamps.
    ///
    /// ```rust
    /// # use someday::*;
    /// // Timestamp is different, data is same.
    /// let commit_1 = Commit { timestamp: 0, data: "" };
    /// let commit_2 = Commit { timestamp: 1, data: "" };
    /// assert!(commit_1.diff_timestamp(&commit_2));
    ///
    /// // Timestamp is same, data is different.
    /// let commit_3 = Commit { timestamp: 0, data: "" };
    /// let commit_4 = Commit { timestamp: 0, data: "a" };
    /// assert!(!commit_3.diff_timestamp(&commit_4));
    /// ```
    pub const fn diff_timestamp(&self, other: &Self) -> bool {
        self.timestamp != other.timestamp
    }

    #[inline]
    /// If there is a difference in `self` & `other`'s timestamps.
    ///
    /// ```rust
    /// # use someday::*;
    /// // Timestamp is different, data is same.
    /// let commit_1 = Commit { timestamp: 0, data: "a" };
    /// let commit_2 = Commit { timestamp: 1, data: "a" };
    /// assert!(!commit_1.diff_data(&commit_2));
    ///
    /// // Timestamp is same, data is different.
    /// let commit_3 = Commit { timestamp: 0, data: "a" };
    /// let commit_4 = Commit { timestamp: 0, data: "b" };
    /// assert!(commit_3.diff_data(&commit_4));
    /// ```
    pub fn diff_data(&self, other: &Self) -> bool
    where
        T: PartialEq<T>,
    {
        self.data != other.data
    }

    #[inline]
    /// If `self`'s timestamp is ahead of `other`'s timestamp.
    ///
    /// ```rust
    /// # use someday::*;
    /// let commit_1 = Commit { timestamp: 0, data: "" };
    /// let commit_2 = Commit { timestamp: 1, data: "" };
    /// assert!(!commit_1.ahead(&commit_2));
    ///
    /// let commit_3 = Commit { timestamp: 2, data: "" };
    /// let commit_4 = Commit { timestamp: 1, data: "" };
    /// assert!(commit_3.ahead(&commit_4));
    ///
    /// let commit_5 = Commit { timestamp: 2, data: "" };
    /// let commit_6 = Commit { timestamp: 2, data: "" };
    /// assert!(!commit_5.ahead(&commit_6));
    /// ```
    pub const fn ahead(&self, other: &Self) -> bool {
        self.timestamp > other.timestamp
    }

    #[inline]
    /// If `self`'s timestamp is behind of `other`'s timestamp.
    ///
    /// ```rust
    /// # use someday::*;
    /// let commit_1 = Commit { timestamp: 0, data: "" };
    /// let commit_2 = Commit { timestamp: 1, data: "" };
    /// assert!(commit_1.behind(&commit_2));
    ///
    /// let commit_3 = Commit { timestamp: 2, data: "" };
    /// let commit_4 = Commit { timestamp: 1, data: "" };
    /// assert!(!commit_3.behind(&commit_4));
    ///
    /// let commit_5 = Commit { timestamp: 2, data: "" };
    /// let commit_6 = Commit { timestamp: 2, data: "" };
    /// assert!(!commit_5.behind(&commit_6));
    /// ```
    pub const fn behind(&self, other: &Self) -> bool {
        self.timestamp < other.timestamp
    }
}

//---------------------------------------------------------------------------------------------------- Commit Trait
impl<T: Clone> TryFrom<CommitRef<T>> for Commit<T> {
    type Error = CommitRef<T>;

    #[inline]
    /// This cheaply acquires ownership of a shared [`CommitRef`]
    /// if you are the only one holding onto it.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::from("hello"));
    ///
    /// let commit_ref = r.head();
    /// // Force the `Writer` to advance onto the next commit.
    /// w.add_commit_push(|_, _| {});
    ///
    /// // Now there's only 1 strong count on `commit_ref`,
    /// // this can be turned into an owned commit.
    /// let commit_owned: Commit<String> = commit_ref.try_into().unwrap();
    /// ```
    fn try_from(commit: CommitRef<T>) -> Result<Self, Self::Error> {
        Arc::try_unwrap(commit)
    }
}

impl<T> std::fmt::Display for Commit<T>
where
    T: Clone + std::fmt::Display,
{
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::from("hello"));
    ///
    /// let display: String = format!("{}", r.head());
    /// assert_eq!(display, "hello");
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.data, f)
    }
}

//---------------------------------------------------------------------------------------------------- CommitRef
/// Cheaply cloneable snapshot of some data `T` and its [`Timestamp`]
///
/// This is a [`Commit`] of data received from operations
/// like [`Writer::head()`] and [`Reader::head()`].
///
/// It is shared data, and cheaply [`Clone`]-able.
///
/// This is just an alias for [`Arc<Commit<T>>`].
pub type CommitRef<T> = Arc<Commit<T>>;

//---------------------------------------------------------------------------------------------------- CommitRef Trait impl
impl<T: Clone> From<&Reader<T>> for CommitRef<T> {
    #[inline]
    /// Calls [`Reader::head`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, _) = someday::new(String::from("hello"));
    ///
    /// let commit_ref: CommitRef<String> = (&r).into();
    /// assert_eq!(commit_ref.data, "hello");
    /// ```
    fn from(reader: &Reader<T>) -> Self {
        reader.head()
    }
}
