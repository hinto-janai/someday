//! Free functions.

//---------------------------------------------------------------------------------------------------- Use
use crate::{
	reader::Reader,
	writer::Writer,
	commit::{Commit,CommitOwned,CommitRef},
	timestamp::Timestamp,
};
use arc_swap::ArcSwapAny;
use std::{
	sync::{Arc,atomic::AtomicBool},
	collections::BTreeMap,
};

//---------------------------------------------------------------------------------------------------- Free functions
#[inline]
#[must_use]
/// Create a new [`Reader`] & [`Writer`] pair.
///
/// See their documentation for writing and reading functions.
///
/// ## Example
/// ```rust
/// let (reader, mut writer) = someday::new::<String>("hello world!".into());
///
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.data_remote(), "hello world!");
/// ```
pub fn new<T: Clone>(data: T) -> (Reader<T>, Writer<T>) {
	let writer = new_inner(CommitOwned { data, timestamp: 0 });
	(writer.reader(), writer)
}

#[inline]
#[must_use]
/// Create a new [`Reader`] & [`Writer`] pair from `T::default()`.
///
/// ## Example
/// ```rust
/// let (reader, mut writer) = someday::default::<usize>();
///
/// assert_eq!(*writer.data(), 0);
/// assert_eq!(*writer.data_remote(), 0);
/// ```
pub fn default<T: Clone + Default>() -> (Reader<T>, Writer<T>) {
	let writer = new_inner(CommitOwned { data: T::default(), timestamp: 0 });
	(writer.reader(), writer)
}

#[inline]
#[must_use]
/// Create a new [`Reader`] & [`Writer`] pair from a [`Commit`].
///
/// This allows you to modify the starting [`Timestamp`],
/// as you can set your `Commit`'s timestamp to any value.
///
/// (Although, setting it to [`usize::MAX`] will cause
/// panics if/when the timestamp gets updated).
///
/// The input `Commit` can either be:
/// - [`CommitOwned<T>`] where this function will take the data as it, or
/// - [`CommitRef<T>`] where this function will _attempt_ to acquire the data
/// if there are no other strong references to it. It will [`Clone`] otherwise.
///
/// ## Example
/// ```rust
/// # use someday::*;
/// let commit = CommitOwned {
///     data: String::from("hello world!"),
///     timestamp: 123,
/// };
/// let (reader, mut writer) = someday::from_commit(commit);
///
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.data_remote(), "hello world!");
/// assert_eq!(writer.timestamp(), 123);
/// assert_eq!(writer.timestamp_remote(), 123);
/// ```
///
/// # Timestamp > [`usize::MAX`]
/// Note that [`Writer`] will start panicking
/// if the `Timestamp` surpasses [`usize::MAX`].
///
/// It should not be set to an extremely high value.
///
/// ```rust,should_panic
/// # use someday::*;
/// let commit = CommitOwned {
///     data: String::from("hello world!"),
///     timestamp: usize::MAX,
/// };
/// let (reader, mut writer) = someday::from_commit(commit);
///
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.timestamp(), usize::MAX);
///
/// // This panics on overflow in debug,
/// // and wraps in release mode.
/// # if !cfg!(debug_assertions) { panic!() };
/// # // This must be added since CI uses --release.
/// writer.add_commit(|_, _| {});
/// ```
pub fn from_commit<T: Clone, C: Commit<T>>(commit: C) -> (Reader<T>, Writer<T>) {
	let writer = new_inner(commit.into_commit_owned());
	(writer.reader(), writer)
}

/// Inner function for constructors.
pub(crate) fn new_inner<T: Clone>(local: CommitOwned<T>) -> Writer<T> {
	/// The default `Vec` capacity for the
	/// `Patch`'s when using using `new()`.
	const INIT_VEC_CAP: usize = 16;

	let remote = Arc::new(local.clone());
	let arc    = Arc::new(ArcSwapAny::new(Arc::clone(&remote)));

	Writer {
		local: Some(local),
		remote,
		arc,
		patches: Vec::with_capacity(INIT_VEC_CAP),
		patches_old: Vec::with_capacity(INIT_VEC_CAP),
		tags: BTreeMap::new(),
	}
}