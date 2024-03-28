//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
    borrow::{Borrow, BorrowMut},
    ops::{Deref, DerefMut},
};

use crate::{info::CommitInfo, patch::Patch, writer::Writer, Timestamp};

#[allow(unused_imports)] // docs
use crate::Reader;
#[allow(unused_imports)] // docs
use std::sync::{Arc, Mutex};

//---------------------------------------------------------------------------------------------------- Tx
/// Mutate the data `T` _directly_.
///
/// This structure is returned by [`Writer::tx`], and can be seen as a
/// temporary handle for mutating your data `T`.
///
/// ## Mutable access to `T`
/// `Transaction` gives _direct_ mutable access to the `Writer`'s inner data `T` via:
/// - [`Transaction::data_mut`]
/// - [`Transaction::deref_mut`]
/// - [`Transaction::borrow_mut`]
/// - [`Transaction::as_mut`]
///
/// Each call to any of these functions will increment the [`Timestamp`]
/// by `1`, regardless if the data was changed or not.
///
/// ## [`Drop`]
/// After `Transaction` has been [`drop()`]'ed:
/// 1. All previous `Patch`'s get cleared
/// 2. A `Patch` that simply clones all the data gets added
///
/// It is worth noting that this only happens if a
/// mutable reference is created (even if it is not used).
///
/// If [`Transaction::sync_patch()`] is used, it allows you
/// to specify the `Patch` that actually syncs the data.
///
/// By default, this is [`Patch::CLONE`], although, if there
/// are cheaper ways to de-duplicate data without cloning, that
/// could be used instead, e.g:
///
/// ```rust
/// # use someday::*;
/// let (r, mut w) = someday::new(Vec::<&str>::new());
///
/// let mut tx = w.tx();
/// tx.push("hello");
/// tx.push(" ");
/// tx.push("world");
/// tx.push("!");
///
/// tx.sync_patch(Patch::Ptr(|w, r| {
///     // Instead of `clone()`'ing the data,
///     // just copy over the data to the current
///     // `Vec` - this way we aren't discarding
///     // the already allocated memory on this side.
///     w.clear();
///     w.extend_from_slice(r);
/// }));
/// drop(tx);
/// let i = w.push();
///
/// assert_eq!(w.data().as_slice(), ["hello", " ", "world", "!"]);
/// assert_eq!(w.timestamp(), 4);
/// assert_eq!(r.head().data.as_slice(), ["hello", " ", "world", "!"]);
/// assert_eq!(r.head().timestamp, 4);
/// ```
///
/// ## `Transaction` vs `Patch`
/// Using `Transaction` instead of `Patch` when you are
/// just cloning data anyway may be preferred as it avoids:
/// - Potential use of [`Box`]/[`Arc`]
/// - Overhead of storing/re-applying `Patch`'s
///
/// `Patch`'s allow for turning cheaply reclaimed old
/// [`Reader`] data back into a viable copy, however, if your `Patch`
/// is simply cloning the data anyway, `Transaction` makes more sense.
///
/// ## ⚠️ `Patch` guardrails
/// `Patch` exists as a means to make sure data is properly synced
/// when reclaiming old data - `Transaction` slightly removes these
/// guardrails as it allows you to define the way data gets synced.
///
/// Passing [`Patch::NOTHING`] to [`Transaction::sync_patch`] is valid,
/// but will most inevitably leave your data in an invalid, unsynced state.
///
/// `Transaction` gives you more control if you know what you're doing
/// (e.g, we'll be cloning later anyway, so these intermediate patches don't matter)
/// but comes at the risk of this unsynced behavior, so be careful.
///
/// ## Example
/// ```rust
/// # use someday::*;
/// // Create `Reader/Writer` pair of a `String`.
/// let (reader, mut writer) = someday::new(String::new());
///
/// // Open up a transaction.
/// let mut tx: Transaction<'_, String> = writer.tx();
/// // Each one of these is silently calling `deref_mut()`
/// // into the target `&mut String`, which increments the
/// // timestamp each call, so in total, 4 times.
/// tx.push_str("hello");
/// tx.push_str(" ");
/// tx.push_str("world");
/// tx.push_str("!");
///
/// // We started with a timestamp of 0,
/// // and after mutating a bit, are now at 4.
/// assert_eq!(tx.original_timestamp(), 0);
/// assert_eq!(tx.current_timestamp(), 4);
///
/// // Finish the transaction by dropping,
/// // we don't want the `CommitInfo` data
/// // from `Transaction::commit()`.
/// drop(tx);
/// // We can see dropping the `Transaction` added
/// // a `Patch` - this just clones the data.
/// assert_eq!(writer.committed_patches().len(), 1);
///
/// // Our changes were applied
/// // to the `Writer` data directly.
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.timestamp(), 4);
/// // Although, we haven't `push()`'ed yet,
/// // so `Reader`'s still don't see changes.
/// assert_eq!(reader.head().data, "");
/// assert_eq!(reader.head().timestamp, 0);
///
/// // `Reader`'s can see our changes after `push()`.
/// writer.push();
/// assert_eq!(reader.head().data, "hello world!");
/// assert_eq!(reader.head().timestamp, 4);
/// ```
pub struct Transaction<'writer, T: Clone> {
    /// TODO
    pub(crate) writer: &'writer mut Writer<T>,
    /// TODO
    pub(crate) original_timestamp: Timestamp,
    /// TODO
    pub(crate) sync_patch: Patch<T>,
}

impl<'writer, T: Clone> Transaction<'writer, T> {
    /// Create a new [`Transaction`] associated with a [`Writer`].
    ///
    /// This is the same as [`Writer::tx`].
    pub fn new(writer: &'writer mut Writer<T>) -> Transaction<'writer, T> {
        Self {
            original_timestamp: writer.timestamp(),
            writer,
            sync_patch: Patch::CLONE,
        }
    }

    #[must_use]
    /// Immutably borrow the [`Writer`]'s data `T`.
    ///
    /// This will not increment the [`Timestamp`].
    pub fn data(&self) -> &T {
        &self.writer.local_as_ref().data
    }

    /// Mutably borrow the [`Writer`]'s data `T`.
    ///
    /// Each call to this function will increment the [`Timestamp`]
    /// by `1`, regardless if the data was changed or not.
    ///
    /// All mutable borrow trait functions
    /// use this function internally, which means
    /// they also increase the timestamp.
    pub fn data_mut(&mut self) -> &mut T {
        // Increment local timestamp assuming
        // each `deref_mut()` will actually mutate
        // the inner value.
        let commit = self.writer.local_as_mut();
        commit.timestamp += 1;

        &mut commit.data
    }

    #[must_use]
    /// Immutably borrow the [`Writer`]'s associated with this [`Transaction`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut writer) = someday::new(String::new());
    ///
    /// let mut tx = writer.tx();
    /// let writer = tx.writer();
    ///
    /// // We can access anything even while
    /// // a `Transaction` is in-scope.
    /// assert!(writer.data().is_empty());
    ///
    /// // But not after it is actually being used.
    /// tx.push_str("");
    /// // writer.head();
    /// ```
    pub fn writer(&self) -> &Writer<T> {
        self.writer
    }

    #[must_use]
    /// Get the original [`Timestamp`] of when this [`Transaction`] was created.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut writer) = someday::new(String::new());
    ///
    /// let mut tx = writer.tx();
    /// tx.push_str(""); // 1
    /// tx.push_str(""); // 2
    /// tx.push_str(""); // 3
    /// assert_eq!(tx.original_timestamp(), 0);
    ///
    /// drop(tx);
    /// assert_eq!(writer.timestamp(), 3);
    ///
    /// let tx = writer.tx();
    /// assert_eq!(tx.original_timestamp(), 3);
    /// ```
    pub const fn original_timestamp(&self) -> Timestamp {
        self.original_timestamp
    }

    #[must_use]
    /// Get the current [`Timestamp`] of the [`Writer`]
    /// associated with this [`Transaction`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut writer) = someday::new(String::new());
    ///
    /// let mut tx = writer.tx();
    /// tx.push_str(""); // 1
    /// tx.push_str(""); // 2
    /// tx.push_str(""); // 3
    ///
    /// assert_eq!(tx.current_timestamp(), 3);
    /// assert_eq!(tx.original_timestamp(), 0);
    /// ```
    pub const fn current_timestamp(&self) -> Timestamp {
        self.writer.timestamp()
    }

    #[must_use]
    /// Return information about the changes made
    /// and complete the [`Transaction`].
    ///
    /// This is the same as [`drop()`]'ing the `Transaction`,
    /// except it will return a [`CommitInfo`].
    ///
    /// [`CommitInfo::patches`] in this case will represent
    /// how many times the `Transaction` was mutably referenced.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut writer) = someday::new(String::new());
    ///
    /// let mut tx = writer.tx();
    /// tx.push_str(""); // 1
    /// tx.push_str(""); // 2
    /// tx.push_str(""); // 3
    /// let commit_info = tx.commit();
    ///
    /// assert_eq!(commit_info.patches, 3);
    /// ```
    pub fn commit(self) -> CommitInfo {
        CommitInfo {
            patches: self
                .current_timestamp()
                .saturating_sub(self.original_timestamp),
            timestamp_diff: self
                .current_timestamp()
                .saturating_sub(self.writer.timestamp_remote()),
        }

        /* drop code */
    }

    /// Customize the synchronization function used by [`Transaction`].
    ///
    /// By default, dropping `Transaction` will add a [`Patch::CLONE`]
    /// to synchronize the `Reader`'s side of the data (if reclaimed).
    ///
    /// This function allows you to change that to a [`Patch`] of your choice.
    ///
    /// # ⚠️ Non-deterministic `Patch`
    /// As noted in [`Patch`] and [`Transaction`] documentation, this `Patch`
    /// must be deterministic and not cause the `Writer/Reader` data to get
    /// out-of-sync.
    ///
    /// **However**, under the circumstance where you eventually _will_ sync
    /// afterwards, `sync_patch` could technically break these rules, for example:
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::new());
    ///
    /// // Open up a `Transaction`
    /// // and start mutating `T`...
    /// let mut tx = w.tx();
    /// tx.push_str("hello");
    /// tx.push_str(" ");
    /// tx.push_str("world");
    ///
    /// // ⚠️ Kinda dangerous...!
    /// // This function is supposed to sync
    /// // our data, but instead it...
    /// tx.sync_patch(Patch::Ptr(|_, _| {
    ///     /* ...does nothing! */
    /// }));
    ///
    /// drop(tx);
    /// assert_eq!(w.data(), "hello world");
    ///
    /// // But... it's okay, we're going to
    /// // be adding a synchronization Patch here.
    /// let mut tx = w.tx();
    /// tx.push_str("!");
    /// drop(tx); // <- `Patch::CLONE` gets added by default.
    ///
    /// assert_eq!(w.data(), "hello world!");
    /// assert_eq!(w.committed_patches().len(), 1); // <- `Patch::CLONE`
    ///
    /// // Now, this push will use that clone patch
    /// // to sync the data, everything is okay.
    /// w.push();
    /// assert_eq!(w.data(), "hello world!");
    /// assert_eq!(w.timestamp(), 4);
    /// assert_eq!(r.head().data, "hello world!");
    /// assert_eq!(r.head().timestamp, 4);
    /// ```
    pub fn sync_patch(&mut self, sync_patch: Patch<T>) -> Patch<T> {
        std::mem::replace(&mut self.sync_patch, sync_patch)
    }

    /// Attempt to abort the `Transaction`.
    ///
    /// This cancels the `Transaction` and returns `Ok(())`
    /// if no mutable references to `T` were created.
    ///
    /// # Errors
    /// If a mutable reference to `T` was created with
    /// [`Transaction::data_mut`], this will return `self` back inside [`Err`].
    ///
    /// # Example
    /// ```rust
    /// # use someday::*;
    /// let (_, mut writer) = someday::new(String::new());
    ///
    /// //---------- No changes made, abort is ok
    /// let mut tx = writer.tx();
    /// assert!(tx.abort().is_ok());
    /// assert_eq!(writer.data(), "");
    /// assert_eq!(writer.timestamp(), 0);
    /// assert_eq!(writer.staged().len(), 0);
    ///
    /// //---------- `T` was mutated, abort fails
    /// let mut tx = writer.tx();
    /// tx.push_str("");
    /// assert!(tx.abort().is_err());
    ///
    /// //---------- Mutable reference was created, abort fails
    /// let mut tx = writer.tx();
    /// tx.data_mut();
    /// assert!(tx.abort().is_err());
    /// ```
    pub fn abort(self) -> Result<(), Self> {
        if self.original_timestamp == self.current_timestamp() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

//---------------------------------------------------------------------------------------------------- Drop
impl<T: Clone> Drop for Transaction<'_, T> {
    fn drop(&mut self) {
        // If we made changes, force a `clone` commit.
        if self.original_timestamp != self.current_timestamp() {
            // Clear old patches, they don't matter
            // anymore since we are cloning regardless.
            self.writer.patches_old.clear();

            // Take the sync `Patch`, add it.
            let patch = std::mem::take(&mut self.sync_patch);
            self.writer.patches_old.push(patch);
        }
    }
}

//---------------------------------------------------------------------------------------------------- Trait
impl<T: Clone> Deref for Transaction<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.writer
    }
}

impl<T: Clone> DerefMut for Transaction<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data_mut()
    }
}

impl<T: Clone> Borrow<T> for Transaction<'_, T> {
    #[inline]
    fn borrow(&self) -> &T {
        self.writer.data()
    }
}

impl<T: Clone> BorrowMut<T> for Transaction<'_, T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self.data_mut()
    }
}

impl<T: Clone> AsRef<T> for Transaction<'_, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.writer.data()
    }
}

impl<T: Clone> AsMut<T> for Transaction<'_, T> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.data_mut()
    }
}
