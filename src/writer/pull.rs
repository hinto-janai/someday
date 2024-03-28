//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use crate::{commit::Commit, info::PullInfo, patch::Patch, writer::Writer};

#[allow(unused_imports)] // docs
use crate::{Reader, Timestamp};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// Conditionally overwrite the [`Writer`]'s local [`Commit`] with the current [`Reader`] `Commit`
    ///
    /// If the `Writer` and `Reader` are [`Writer::synced()`], this will return `None`.
    ///
    /// If the `Writer` is ahead of the `Reader`, this will:
    /// - Discard all `Patch`'s that have been already [`commit()`](Writer::commit)'ed
    /// - Keep staged `Patch`'s that haven't been `commit()`
    /// - Return `Some(PullInfo)`
    ///
    /// The [`PullInfo`] object returned is just a container
    /// for some metadata about the [`pull()`](Writer::pull) operation.
    ///
    /// ## Timestamp
    /// If this pull is successful (the `Writer` and `Reader` aren't in sync),
    /// this will reset your `Writer`'s [`Timestamp`] to whatever your `Reader`'s was.
    ///
    /// ## ⚠️ Warning
    /// This overwrites your `Writer`'s data!
    ///
    /// Like a `git pull --force`!
    ///
    /// ```rust
    /// # use someday::{*,info::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Commit local changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("hello")));
    /// w.commit();
    /// assert_eq!(w.head().data, "hello");
    ///
    /// // Reader's sees nothing
    /// assert_eq!(r.head().data, "");
    ///
    /// // Pull from the Reader.
    /// let pull_status: PullInfo<String> = w.pull().unwrap();
    /// assert_eq!(pull_status.old_writer_commit.data, "hello");
    ///
    /// // We're back to square 1.
    /// assert_eq!(w.head().data, "");
    ///
    /// // If we try to pull again, nothing will happen
    /// // since we are already synced with `Reader`s.
    /// assert!(w.pull().is_none());
    /// ```
    pub fn pull(&mut self) -> Option<PullInfo<T>> {
        // Early return if we're synced.
        if self.synced() {
            return None;
        }

        // INVARIANT: if we're not synced, that
        // means `timestamp_diff` is non-zero.
        let commits_reverted = std::num::NonZeroUsize::new(self.timestamp_diff()).unwrap();

        // INVARIANT: `local` must be initialized after push()
        let old_writer_commit = self.local.take().unwrap();
        self.local = Some((*self.remote).clone());

        // Delete old functions, we won't need
        // them anymore since we just overwrote
        // our data anyway.
        self.patches_old.clear();

        Some(PullInfo {
            commits_reverted,
            old_writer_commit,
        })
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// Overwrite the [`Writer`]'s local data with `data`.
    ///
    /// The `Writer`'s old local data is returned.
    ///
    /// All `Patch`'s that have been already [`commit()`](Writer::commit)'ed are discarded ([`Writer::committed_patches()`]).
    ///
    /// Staged `Patch`'s that haven't been [`commit()`](Writer::commit) still kept around ([`Writer::staged()`]).
    ///
    /// A `Patch` that overwrites the data applied with `commit()` would be
    /// equivalent to this convenience function, although, this function will
    /// be slightly cheaper as it avoids cloning an extra time.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Push changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("hello")));
    /// w.commit(); // <- commit 1
    /// w.push();
    ///
    /// assert_eq!(w.timestamp(), 1);
    ///
    /// // Reader's sees them.
    /// assert_eq!(r.head().data, "hello");
    /// assert_eq!(r.head().timestamp, 1);
    ///
    /// // Commit some changes.
    /// w.add(Patch::Ptr(|w, _| *w = "hello".into()));
    /// w.commit(); // <- commit 2
    /// w.add(Patch::Ptr(|w, _| *w = "hello".into()));
    /// w.commit(); // <- commit 3
    /// w.add(Patch::Ptr(|w, _| *w = "hello".into()));
    /// w.commit(); // <- commit 4
    /// assert_eq!(w.committed_patches().len(), 3);
    ///
    /// // Overwrite the Writer with arbitrary data.
    /// let old_data = w.overwrite(String::from("world")); // <- commit 5
    /// assert_eq!(old_data.data, "hello");
    /// assert_eq!(w.data(), "world");
    /// // Committed functions were deleted, but 1 patch is leftover.
    /// // This is an automatically inserted patch that makes the
    /// // reclaimed `Reader`'s `.clone()` the new data.
    /// assert_eq!(w.committed_patches().len(), 1);
    ///
    /// // `Reader`'s still don't see changes.
    /// assert_eq!(r.head().data, "hello");
    ///
    /// // Push that change.
    /// w.push();
    ///
    /// // Now `Reader`s see change.
    /// assert_eq!(r.head().data, "world");
    ///
    /// // 5 commits total.
    /// assert_eq!(w.timestamp(), 5);
    /// assert_eq!(r.head().timestamp, 5);
    /// ```
    ///
    /// ## Timestamp
    /// This increments the `Writer`'s local `Timestamp` by `1`.
    pub fn overwrite(&mut self, data: T) -> Commit<T> {
        // Delete old functions, we won't need
        // them anymore since we just overwrote
        // our data anyway.
        self.patches_old.clear();

        // INVARIANT: `local` must be initialized after push()
        let timestamp = self.timestamp() + 1;
        let old_data = self.local.take().unwrap();

        self.local = Some(Commit { timestamp, data });

        // Add a `Patch` that clones the new data
        // to the _old_ patches, meaning they are
        // being applied to reclaimed `Reader` data.
        self.patches_old.push(Patch::Ptr(|w, r| {
            // INVARIANT/cool_trick:
            // This is _not_ a `FnOnce()`, so we cannot take `data` by value and do `*w = data;`.
            // But, we are aware that this is `patches_old`, and by the time `push()` gets called,
            // the `r` in this scope will actually be `data`.
            //
            // This means we can just mimic an overwrite by calling `clone()`.
            // This also means we don't have to move `data` within here and do Box stuff.
            //
            //  old_reader_data
            //  |
            //  |   current_reader_head (which will be the passed T on `push()`)
            //  v   v
            *w = r.clone();
        }));

        old_data
    }
}
