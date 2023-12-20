//! Metadata resulting from common [`Writer`] operations
//!
//! These are simple container structs that hold
//! information about [`Writer`] operations.

//---------------------------------------------------------------------------------------------------- Use
use crate::{Timestamp, CommitOwned, Writer, Reader, Commit};

//---------------------------------------------------------------------------------------------------- Info
/// Metadata about a [`Writer::commit()`]
///
/// This is a container for holding the metadata
/// [`Writer`] commit operations produce.
///
/// It is returned from commit-like functions.
pub struct CommitInfo {
	/// How many function's were applied in this [`Commit`]?
	pub functions: usize,
	/// How many [`Commit`]'s is the [`Writer`] now ahead of
	/// compared to the [`Reader`]'s latest head [`Commit`]?
	pub timestamp_diff: usize,
}

/// Metadata about a [`Writer::push()`]
///
/// This is a container for holding the metadata
/// [`Writer`] push operations produce.
///
/// It is returned from push-like functions.
pub struct PushInfo {
	/// The new [`Timestamp`] of the head [`Commit`]
	pub timestamp: Timestamp,
	/// How many [`Commit`]'s were pushed?
	pub commits: usize,
	/// Did the [`Writer`] get to cheaply reclaim old
	/// data and re-apply the `Patch`'s?
	///
	/// If this is `false`, it means the `Writer`
	/// expensively cloned the data directly.
	pub reclaimed: bool,
}

/// Metadata about a [`Writer::pull()`]
///
/// This is a container for holding the metadata
/// [`Writer`] pull operations produce.
///
/// It is returned from pull-like functions.
pub struct PullInfo<T> {
	/// How many [`Commit`]'s did the [`Writer`] go backwards?
	///
	/// For example, if the [`Writer`]'s [`Timestamp`] is `5`
	/// and they [`Writer::pull()`]'ed when the [`Reader`]'s
	/// [`Timestamp`] was `3`, this field would hold `2`.
	pub commits_reverted: usize,
	/// The fully owned local data the [`Writer`] had before
	/// replacing it with the [`Reader`]'s data.
	pub old_writer_data: CommitOwned<T>,
}

/// A variety of status info about the [`Writer`] and [`Reader`]
///
/// This is a bag of various metadata about the current
/// state of the [`Writer`] and [`Reader`].
///
/// It is returned from [`Writer::status()`].
///
/// If you only need 1 or a few of these fields, consider
/// using their individual methods instead.
pub struct StatusInfo<'a, T, Patch> {
	/// [`Writer::staged`]
	pub staged_functions: &'a Vec<Patch>,
	/// [`Writer::committed_functions`]
	pub committed_functions: &'a Vec<Patch>,
	/// [`Writer::head`]
	pub head: &'a CommitOwned<T>,
	/// [`Writer::head_remote`]
	pub head_remote: &'a CommitOwned<T>,
	/// [`Writer::head_readers`]
	pub head_readers: usize,
	/// [`Writer::reader_count`]
	pub reader_count: usize,
	/// [`Writer::timestamp`]
	pub timestamp: Timestamp,
	/// [`Writer::timestamp_remote`]
	pub timestamp_remote: Timestamp,
}