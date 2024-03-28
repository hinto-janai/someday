//! Free functions.

//---------------------------------------------------------------------------------------------------- Use
use crate::{commit::Commit, reader::Reader, writer::Writer};
use arc_swap::ArcSwapAny;
use std::sync::{atomic::AtomicBool, Arc};

#[allow(unused_imports)] // docs
use crate::{CommitRef, Timestamp};

//---------------------------------------------------------------------------------------------------- Free functions
/// The default `Vec` capacity for the
/// `Patch`'s when using using `new()`.
pub(crate) const INIT_VEC_CAP: usize = 16;

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
    let writer = new_inner(Commit { data, timestamp: 0 });
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
    let writer = new_inner(Commit {
        data: T::default(),
        timestamp: 0,
    });
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
/// - [`Commit<T>`] where this function will take the data as it, or
/// - [`CommitRef<T>`] where this function will _attempt_ to acquire the data
/// if there are no other strong references to it. It will [`Clone`] otherwise.
///
/// ## Example
/// ```rust
/// # use someday::*;
/// let commit = Commit {
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
pub fn from_commit<T: Clone>(commit: Commit<T>) -> (Reader<T>, Writer<T>) {
    let writer = new_inner(commit);
    (writer.reader(), writer)
}

/// Inner function for constructors.
pub(crate) fn new_inner<T: Clone>(local: Commit<T>) -> Writer<T> {
    let remote = Arc::new(local.clone());
    let arc = Arc::new(ArcSwapAny::new(Arc::clone(&remote)));

    Writer {
        token: Arc::new(AtomicBool::new(false)).into(),
        local: Some(local),
        remote,
        arc,
        patches: Vec::with_capacity(INIT_VEC_CAP),
        patches_old: Vec::with_capacity(INIT_VEC_CAP),
    }
}
