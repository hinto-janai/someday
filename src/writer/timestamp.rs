//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use

use crate::{writer::Writer, Timestamp};

#[allow(unused_imports)] // docs
use crate::{Commit, CommitRef, Reader};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// If the [`Writer`]'s local [`Commit`] is different than the [`Reader`]'s
    ///
    /// Compares the `Commit` that the `Reader`'s can
    /// currently access with the `Writer`'s current local `Commit`.
    ///
    /// This returns `true` if either the:
    /// - [`Timestamp`] is different
    /// - Data is different
    ///
    /// ## Purpose
    /// In correct scenarios, the `Writer`'s and `Reader`'s `Timestamp`'s
    /// should be all that is needed to indicate if the data is different or not.
    ///
    /// However, if your `Patch` functions are non-determistic,
    /// the data may get out of sync.
    ///
    /// Thus, this function is mostly meant to be used for debugging purposes.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::sync::*;
    /// // Create a non-deterministic `Writer/Reader`
    /// // out-of-sync issue.
    /// static STATE: Mutex<usize> = Mutex::new(1);
    /// let (_, mut w) = someday::new::<usize>(0);
    /// w.add(Patch::boxed(move |w, _| {
    ///     let mut state = STATE.lock().unwrap();
    ///     *state *= 10; // 1*10 the first time, 10*10 the second time...
    ///     *w = *state;
    /// }));
    /// w.commit();
    /// w.push();
    ///
    /// // Same timestamps...
    /// assert_eq!(w.timestamp(), w.reader().head().timestamp);
    ///
    /// // ⚠️ Out of sync data!
    /// assert_eq!(*w.data(), 100);
    /// assert_eq!(w.reader().head().data, 10);
    ///
    /// // But, this function tells us the truth.
    /// assert_eq!(w.diff(), true);
    /// ```
    pub fn diff(&self) -> bool
    where
        T: PartialEq<T>,
    {
        self.local_as_ref().diff(&*self.remote)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// If the [`Writer`]'s local [`Timestamp`] is greater than the [`Reader`]'s `Timestamp`
    ///
    /// Compares the timestamp of the `Reader`'s currently available
    /// data with the `Writer`'s current local timestamp.
    ///
    /// This returns `true` if the `Writer`'s timestamp
    /// is greater than `Reader`'s timestamp (which means
    /// `Writer` is ahead of the `Reader`'s)
    ///
    /// Note that this does not check the data itself, only the `Timestamp`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Commit 10 times but don't push.
    /// for i in 0..10 {
    ///     w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///     w.commit();
    /// }
    ///
    /// // Writer at timestamp 10.
    /// assert_eq!(w.timestamp(), 10);
    ///
    /// // Reader at timestamp 0.
    /// assert_eq!(r.head().timestamp, 0);
    ///
    /// // Writer is ahead of the Reader's.
    /// assert!(w.ahead());
    /// ```
    pub fn ahead(&self) -> bool {
        self.local_as_ref().ahead(&*self.remote)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// If the [`Writer`]'s local [`Timestamp`] is greater than an arbitrary [`Commit`]'s `Timestamp`
    ///
    /// This takes any type of `Commit`, so either [`CommitRef`] or [`Commit`] can be used as input.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut w) = someday::new::<String>("".into());
    ///
    /// // Commit 10 times.
    /// for i in 0..10 {
    ///     w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///     w.commit();
    /// }
    /// // At timestamp 10.
    /// assert_eq!(w.timestamp(), 10);
    ///
    /// // Create fake `Commit`
    /// let fake_commit = Commit {
    ///     timestamp: 1,
    ///     data: String::new(),
    /// };
    ///
    /// // Writer is ahead of that commit.
    /// assert!(w.ahead_of(&fake_commit));
    /// ```
    pub const fn ahead_of(&self, commit: &Commit<T>) -> bool {
        self.local_as_ref().ahead(commit)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// If the [`Writer`]'s local [`Timestamp`] is less than an arbitrary [`Commit`]'s `Timestamp`
    ///
    /// This takes any type of `Commit`, so either [`CommitRef`] or [`Commit`] can be used as input.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut w) = someday::new::<String>("".into());
    ///
    /// // At timestamp 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Create fake `Commit`
    /// let fake_commit = Commit {
    ///     timestamp: 1000,
    ///     data: String::new(),
    /// };
    ///
    /// // Writer is behind that commit.
    /// assert!(w.behind(&fake_commit));
    /// ```
    pub const fn behind(&self, commit: &Commit<T>) -> bool {
        self.local_as_ref().behind(commit)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// Get the current [`Timestamp`] of the [`Writer`]'s local [`Commit`]
    ///
    /// This returns the number indicating the `Writer`'s data's version.
    ///
    /// This number starts at `0`, increments by `1` every time a [`Writer::commit()`]
    /// -like operation is called, and it will never be less than the [`Reader`]'s `Timestamp`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // At timestamp 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Commit some changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// // At timestamp 1.
    /// assert_eq!(w.timestamp(), 1);
    /// // We haven't pushed, so Reader's
    /// // are still at timestamp 0.
    /// assert_eq!(r.head().timestamp, 0);
    /// ```
    pub const fn timestamp(&self) -> Timestamp {
        self.local_as_ref().timestamp
    }

    #[inline]
    /// Get the current [`Timestamp`] of the [`Reader`]'s "head" [`Commit`]
    ///
    /// This returns the number indicating the `Reader`'s data's version.
    ///
    /// This will never be greater than the [`Writer`]'s timestamp.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // At timestamp 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Commit some changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// // Writer is at timestamp 1.
    /// assert_eq!(w.timestamp(), 1);
    /// // We haven't pushed, so Reader's
    /// // are still at timestamp 0.
    /// assert_eq!(r.head().timestamp, 0);
    ///
    /// // Push changes
    /// w.push();
    ///
    /// // Readers are now up-to-date.
    /// assert_eq!(r.head().timestamp, 1);
    /// ```
    pub fn timestamp_remote(&self) -> Timestamp {
        self.remote.timestamp
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// Get the difference between the [`Writer`]'s and [`Reader`]'s [`Timestamp`]
    ///
    /// This returns the number indicating how many commits the
    /// `Writer` is ahead on compared to the `Reader`'s.
    ///
    /// In other words, it is: `writer_timestamp - reader_timestamp`
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // At timestamp 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Push 1 change.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    /// w.push();
    ///
    /// // Commit 5 changes locally.
    /// for i in 0..5 {
    ///     w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///     w.commit();
    /// }
    ///
    /// // Writer is at timestamp 5.
    /// assert_eq!(w.timestamp(), 6);
    /// // Reader's are still at timestamp 1.
    /// assert_eq!(r.head().timestamp, 1);
    ///
    /// // The difference is 5.
    /// assert_eq!(w.timestamp_diff(), 5);
    /// ```
    pub fn timestamp_diff(&self) -> usize {
        self.local_as_ref().timestamp - self.remote.timestamp
    }

    #[inline]
    /// Is the [`Writer`]'s and [`Reader`]'s [`Timestamp`] the same?
    ///
    /// This returns `true` if the `Writer` and `Reader`'s timestamp
    /// are the same, indicating they have same data and are in-sync.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // At timestamp 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Push 1 change.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    /// w.push();
    ///
    /// // Commit 5 changes locally.
    /// for i in 0..5 {
    ///     w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///     w.commit();
    /// }
    ///
    /// // Writer is at timestamp 5.
    /// assert_eq!(w.timestamp(), 6);
    /// // Reader's are still at timestamp 1.
    /// assert_eq!(r.head().timestamp, 1);
    ///
    /// // They aren't in sync.
    /// assert_eq!(w.synced(), false);
    /// // Now they are.
    /// w.push();
    /// assert_eq!(w.synced(), true);
    /// ```
    pub fn synced(&self) -> bool {
        self.timestamp_diff() == 0
    }
}
