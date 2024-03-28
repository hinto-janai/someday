//! Metadata resulting from common [`Writer`] operations.
//!
//! These are simple container structs that hold
//! information about [`Writer`] operations.

//---------------------------------------------------------------------------------------------------- Use
use crate::{
    commit::{Commit, CommitRef},
    patch::Patch,
    Timestamp,
};
#[allow(unused_imports)] // docs
use crate::{Reader, Writer};
use std::num::NonZeroUsize;

//---------------------------------------------------------------------------------------------------- Info
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Metadata about a [`Writer::commit()`]
///
/// This is a container for holding the metadata
/// [`Writer`] commit operations produce.
///
/// It is returned from commit-like functions.
pub struct CommitInfo {
    /// How many patches's were applied in this [`Commit`]?
    pub patches: usize,
    /// How many [`Commit`]'s is the [`Writer`] now ahead of
    /// compared to the [`Reader`]'s latest head [`Commit`]?
    pub timestamp_diff: usize,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Metadata about a [`Writer::push()`]
///
/// This is a container for holding the metadata
/// [`Writer`] push operations produce.
///
/// It is returned from push-like functions.
pub struct PushInfo {
    /// The new [`Timestamp`] of the head [`Commit`]
    ///
    /// This will be the same as the [`Writer`]'s local timestamp
    /// if `push()` didn't actually do anything (up-to-date with readers).
    pub timestamp: Timestamp,
    /// How many [`Commit`]'s were pushed?
    ///
    /// This will be `0` if `push()` didn't actually do anything (up-to-date with readers).
    pub commits: usize,
    /// Did the [`Writer`] get to cheaply reclaim old
    /// data and re-apply the `Patch`'s?
    ///
    /// If this is `false`, it means either
    /// - The `Writer` expensively cloned the data directly OR
    /// - `push()` didn't have any changes to push (up-to-date with readers)
    pub reclaimed: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Metadata about a [`Writer::pull()`]
///
/// This is a container for holding the metadata
/// [`Writer`] pull operations produce.
///
/// It is returned from pull-like functions.
pub struct PullInfo<T: Clone> {
    /// How many [`Commit`]'s did the [`Writer`] go backwards?
    ///
    /// For example, if the [`Writer`]'s [`Timestamp`] is `5`
    /// and they [`Writer::pull()`]'ed when the [`Reader`]'s
    /// [`Timestamp`] was `3`, this field would hold `2`.
    pub commits_reverted: std::num::NonZeroUsize,
    /// The owned [`Commit`] the [`Writer`] had before
    /// replacing it with the [`Reader`]'s data.
    pub old_writer_commit: Commit<T>,
}

#[derive(Copy, Clone)]
/// A variety of status info about the [`Writer`] and [`Reader`]
///
/// This is a bag of various metadata about the current
/// state of the [`Writer`] and [`Reader`].
///
/// It is returned from [`Writer::status()`].
///
/// If you only need 1 or a few of these fields, consider
/// using their individual methods instead.
pub struct StatusInfo<'a, T: Clone> {
    /// [`Writer::staged`]
    pub staged_patches: &'a Vec<Patch<T>>,
    /// [`Writer::committed_patches`]
    pub committed_patches: &'a Vec<Patch<T>>,
    /// [`Writer::head`]
    pub head: &'a Commit<T>,
    /// [`Writer::head_remote`]
    pub head_remote: &'a Commit<T>,
    /// [`Writer::head_count`]
    pub head_count: NonZeroUsize,
    /// [`Writer::reader_count`]
    pub reader_count: NonZeroUsize,
    /// [`Writer::timestamp`]
    pub timestamp: Timestamp,
    /// [`Writer::timestamp_remote`]
    pub timestamp_remote: Timestamp,
}

/// The inner structures of a `Writer`, returned by [`Writer::into_inner`].
pub struct WriterInfo<T: Clone> {
    /// The `Writer`'s local data.
    ///
    /// [`Writer::head`].
    pub writer: Commit<T>,

    /// The latest [`Reader`]'s [`Commit`].
    ///
    /// [`Writer::head_remote`].
    pub reader: CommitRef<T>,

    /// The "staged" `Patch`'s that haven't been [`commit()`](Writer::commit)'ed.
    ///
    /// [`Writer::staged`].
    pub staged: Vec<Patch<T>>,

    /// The committed `Patch`'s that haven't been [`push()`](Writer::push)'ed.
    ///
    /// [`Writer::committed_patches`].
    pub committed_patches: Vec<Patch<T>>,
}
