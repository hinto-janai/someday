//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
	sync::{Arc,
		atomic::{
			AtomicBool,
			Ordering,
		},
	},
	time::Duration,
	borrow::Borrow,
	collections::BTreeMap,
	num::NonZeroUsize,
};

use crate::{
	writer::Writer,
	patch::Patch,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
	#[inline]
	/// Store the latest [`Reader`] head [`Commit`] (cheaply)
	///
	/// This stores the latest `Reader` `Commit` into the [`Writer`]'s local storage.
	///
	/// These tags can be inspected later with [`Writer::tags()`].
	///
	/// If `Writer::tag()` is never used, it will never allocate space.
	///
	/// This returns the tagged [`CommitRef`] that was stored.
	///
	/// # Why does this exist?
	/// You could store your own collection of `CommitRef`'s alongside
	/// your `Writer` and achieve similar results, however there are
	/// benefits to `Writer` coming with one built-in:
	///
	/// 1. It logically associates `Commit`'s with a certain `Writer`
	/// 2. The invariant that all `Commit`'s tagged are/were valid `Commit`'s
	/// to both the `Writer` and `Reader` is always upheld as the `Writer`
	/// does not provide mutable access to the inner `Commit` data or [`Timestamp`]'s
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Push a change.
	/// w.add(Patch::Ptr(|w, _| w.push_str("a")));
	/// w.commit();
	/// w.push();
	///
	/// // Tag that change, and clone it (this is cheap).
	/// let tag = CommitRef::clone(w.tag());
	///
	/// // This tag is the same as the Reader's head Commit.
	/// assert_eq!(tag, r.head());
	/// assert_eq!(tag.timestamp(), 1);
	///
	/// // Push a whole bunch changes.
	/// for _ in 0..100 {
	///     w.add(Patch::Ptr(|w, _| w.push_str("b")));
	///     w.commit();
	///     w.push();
	/// }
	/// assert_eq!(w.timestamp(), 101);
	/// assert_eq!(r.timestamp(), 101);
	///
	/// // Writer is still holding onto the tag, so remove it.
	/// let removed_tag = w.tag_remove(tag.timestamp()).unwrap();
	/// assert_eq!(removed_tag, tag);
	///
	/// // Atomically decrements a counter.
	/// drop(removed_tag);
	///
	/// // SAFETY: now we know that we're the
	/// // only ones holding onto this commit.
	/// let inner_data: String = std::sync::Arc::try_unwrap(tag).unwrap().data;
	///
	/// // Now, let's use that old tag to overwrite our current data.
	/// //
	/// // Note that the Writer can _never_ "rebase" and go back in time
	/// // (at least, before the Reader's timestamp).
	/// //
	/// // This overwrite operation is the same as a regular commit,
	/// // it takes the data and adds 1 to the timestamp, it does
	/// // not reset the timestamp.
	/// w.overwrite(inner_data);
	/// assert_eq!(w.timestamp(), 102);
	/// assert_eq!(w.data(), "a");
	/// ```
	pub fn tag(&mut self) -> &CommitRef<T> {
		self.tags.entry(self.remote.timestamp)
			.or_insert_with(|| Arc::clone(&self.remote))
	}

	#[inline]
	/// Return all tagged [`Commit`]'s
	///
	/// This returns a [`BTreeMap`] where the:
	/// - Key is the `Commit`'s [`Timestamp`], and the
	/// - Value is the shared [`CommitRef`] object itself
	///
	/// Mutable access to these tags are restricted in a way
	/// such that these tags are guaranteed to have been valid
	/// `Commit`'s that were [`push()`](Writer::push)'ed to the [`Reader`]'s.
	///
	/// Aka, these tags will never be arbitrary data.
	///
	/// Therefore the `Timestamp` and `CommitRef` data can be relied upon.
	///
	/// These "tags" are created with [`Writer::tag()`].
	///
	/// TODO: doc test.
	pub const fn tags(&self) -> &BTreeMap<Timestamp, CommitRef<T>> {
		&self.tags
	}

	#[inline]
	/// Clear all the stored [`Writer`] tags
	///
	/// This calls [`BTreeMap::clear()`] on this `Writer`'s internal tags.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Push a change.
	/// w.add(Patch::Ptr(|w, _| w.push_str("a")));
	/// w.commit();
	/// w.push();
	///
	/// // Tag that change.
	/// let tag = w.tag();
	/// assert_eq!(*tag, r.head());
	/// assert_eq!(tag.timestamp(), 1);
	///
	/// // Clear all tags.
	/// w.tag_clear();
	/// assert_eq!(w.tags().len(), 0);
	/// ```
	pub fn tag_clear(&mut self) {
		self.tags.clear();
	}

	#[inline]
	/// Run [`std::mem::take()`] on the [`Writer`]'s tags
	///
	/// This will return the old tags and will
	/// replace the `Writer`'s tag with a new empty set.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Tag 100x times.
	/// for i in 0..100 {
	///     w.add(Patch::Ptr(|w, _| w.push_str("a")));
	///     w.commit();
	///     w.push();
	///     w.tag();
	/// }
	///
	/// // Take all tags.
	/// let tags = w.tag_take();
	/// assert_eq!(w.tags().len(), 0);
	/// assert_eq!(tags.len(), 100);
	/// ```
	pub fn tag_take(&mut self) -> BTreeMap<Timestamp, CommitRef<T>> {
		std::mem::take(&mut self.tags)
	}

	/// Retains only the tags specified by the predicate
	///
	/// In other words, remove all tags for which `F` returns false.
	///
	/// The elements are visited in ascending key order.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut writer) = someday::new::<String>("aaa".into());
	///
	/// // Tag this "aaa" commit.
	/// writer.tag();
	///
	/// // Push and tag a whole bunch changes.
	/// for i in 1..100 {
	///     writer.add(Patch::Ptr(|w, _| *w = "bbb".into()));
	///     writer.commit();
	///     writer.push();
	///     writer.tag();
	/// }
	///
	/// assert_eq!(writer.tags().len(), 100);
	///
	/// // Only retain the tags where the
	/// // commit data value is "aaa".
	/// writer.tag_retain(|commit| *commit.data() == "aaa");
	///
	/// // Just 1 tag now.
	/// assert_eq!(writer.tags().len(), 1);
	/// assert_eq!(*writer.tags().get(&0).unwrap().data(), "aaa");
	/// ```
	pub fn tag_retain<F>(&mut self, mut f: F)
	where
		F: FnMut(&CommitRef<T>) -> bool,
	{
		self.tags.retain(|_, commit| f(commit));
	}

	#[inline]
	/// Remove a stored tag from the [`Writer`]
	///
	/// This calls [`BTreeMap::remove()`] on this `Writer`'s internal tags.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut writer) = someday::new::<String>("aaa".into());
	///
	/// let tag = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_remove(tag.timestamp()).unwrap();
	///
	/// assert_eq!(tag, removed);
	/// ```
	pub fn tag_remove(&mut self, timestamp: Timestamp) -> Option<CommitRef<T>> {
		self.tags.remove(&timestamp)
	}

	#[inline]
	/// Removes and returns the oldest tag from the [`Writer`]
	///
	/// The [`CommitRef`] returned is the _oldest_ one (smallest [`Timestamp`]).
	///
	/// This calls [`BTreeMap::pop_first()`] on this `Writer`'s internal tags.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut writer) = someday::new::<String>("aaa".into());
	///
	/// let tag_0 = CommitRef::clone(&writer.tag());
	/// let tag_1 = CommitRef::clone(&writer.tag());
	/// let tag_2 = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_pop_oldest().unwrap();
	///
	/// assert_eq!(tag_0, removed);
	/// ```
	pub fn tag_pop_oldest(&mut self) -> Option<CommitRef<T>> {
		self.tags.pop_first().map(|(_, c)| c)
	}

	#[inline]
	/// Removes and returns the last tag from the [`Writer`]
	///
	/// The [`CommitRef`] returned is the _latest_ one (largest [`Timestamp`]).
	///
	/// This calls [`BTreeMap::pop_last()`] on this `Writer`'s internal tags.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut writer) = someday::new::<String>("aaa".into());
	///
	/// let tag_0 = CommitRef::clone(&writer.tag());
	/// let tag_1 = CommitRef::clone(&writer.tag());
	/// let tag_2 = CommitRef::clone(&writer.tag());
	///
	/// let removed = writer.tag_pop_latest().unwrap();
	///
	/// assert_eq!(tag_2, removed);
	/// ```
	pub fn tag_pop_latest(&mut self) -> Option<CommitRef<T>> {
		self.tags.pop_last().map(|(_, c)| c)
	}
}