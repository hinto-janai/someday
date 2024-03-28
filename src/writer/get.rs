//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{num::NonZeroUsize, sync::Arc};

use crate::{
    commit::{Commit, CommitRef},
    info::StatusInfo,
    patch::Patch,
    reader::Reader,
    transaction::Transaction,
    writer::Writer,
};

#[allow(unused_imports)] // docs
                         // use crate::Commit;

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[inline]
    /// Cheaply construct a [`Reader`] connected to this [`Writer`]
    ///
    /// This creates a new `Reader` that can read all the
    /// data [`push()`](Writer::push)'ed from this `Writer`.
    ///
    /// There is no limit on concurrent `Reader`'s.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // Create 100 more readers.
    /// let readers: Vec<Reader<usize>> = vec![w.reader(); 100];
    /// ```
    pub fn reader(&self) -> Reader<T> {
        Reader {
            arc: Arc::clone(&self.arc),
            token: self.token.clone(),
            cache: None,
        }
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// View the [`Writer`]'s _local_ data
    ///
    /// This is the `Writer`'s local data that may or may
    /// not have been [`push()`](Writer::push)'ed yet.
    ///
    /// [`commit()`](Writer::commit)'ing will apply the
    /// [`add()`](Writer::add)'ed `Patch`'s directly to this data.
    ///
    /// If `push()` is called, this would be the
    /// new data that `Reader`'s would see.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // No changes yet.
    /// assert_eq!(*w.data(), 0);
    /// assert_eq!(r.head().data,  0);
    ///
    /// // Writer commits some changes.
    /// w.add(Patch::Ptr(|w, _| *w += 1));
    /// w.commit();
    ///
    /// //  Writer sees local change.
    /// assert_eq!(*w.data(), 1);
    /// // Reader doesn't see change.
    /// assert_eq!(r.head().data, 0);
    /// ```
    pub const fn data(&self) -> &T {
        &self.local_as_ref().data
    }

    /// Mutate the [`Writer`]'s local data _without_ going through a [`Patch`].
    ///
    /// This function gives you _direct_ access to the
    /// underlying local `T` via a [`Transaction`].
    ///
    /// In order to prevent out-of-sync situations, `Transaction` will
    /// unconditionally add a `Patch` that clones data after it has been [`drop`]'ed.
    ///
    /// This is cheaper than `Patch` if you had already planned to clone data anyway.
    ///
    /// See `Transaction` for more details.
    pub fn tx(&mut self) -> Transaction<'_, T> {
        Transaction::new(self)
    }

    #[inline]
    /// View the latest copy of data [`Reader`]'s have access to
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut w) = someday::new::<usize>(0);
    ///
    /// // Writer commits some changes.
    /// w.add(Patch::Ptr(|w, _| *w += 1));
    /// w.commit();
    ///
    /// // Writer sees local change.
    /// assert_eq!(*w.data(), 1);
    /// // But they haven't been pushed to the remote side
    /// // (Readers can't see them)
    /// assert_eq!(*w.data_remote(), 0);
    /// ```
    pub fn data_remote(&self) -> &T {
        &self.remote.data
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// View the [`Writer`]'s local "head" [`Commit`]
    ///
    /// This is the latest, and local `Commit` from the `Writer`.
    ///
    /// Calling [`commit()`](Writer::commit) would make that new
    /// `Commit` be the return value for this function.
    ///
    /// [`Reader`]'s may or may not see this `Commit` yet.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut w) = someday::new::<usize>(500);
    ///
    /// // No changes yet.
    /// let commit: &Commit<usize> = w.head();
    /// assert_eq!(commit.timestamp, 0);
    /// assert_eq!(commit.data, 500);
    ///
    /// // Writer commits some changes.
    /// w.add(Patch::Ptr(|w, _| *w += 1));
    /// w.commit();
    ///
    /// // Head commit is now changed.
    /// let commit: &Commit<usize> = w.head();
    /// assert_eq!(commit.timestamp, 1);
    /// assert_eq!(commit.data, 501);
    /// ```
    pub const fn head(&self) -> &Commit<T> {
        self.local_as_ref()
    }

    #[inline]
    /// View the [`Reader`]'s latest "head" [`Commit`]
    ///
    /// This is the latest `Commit` the `Reader`'s can see.
    ///
    /// Calling [`push()`](Writer::push) would update the `Reader`'s head `Commit`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, mut w) = someday::new::<usize>(500);
    ///
    /// // No changes yet.
    /// let commit: &Commit<usize> = w.head_remote();
    /// assert_eq!(commit.timestamp, 0);
    /// assert_eq!(commit.data, 500);
    ///
    /// // Writer commits & pushes some changes.
    /// w.add(Patch::Ptr(|w, _| *w += 1));
    /// w.commit();
    /// w.push();
    ///
    /// // Reader's head commit is now changed.
    /// let commit: &Commit<usize> = w.head_remote();
    /// assert_eq!(commit.timestamp, 1);
    /// assert_eq!(commit.data, 501);
    /// ```
    pub fn head_remote(&self) -> &Commit<T> {
        &self.remote
    }

    #[inline]
    /// Cheaply acquire ownership of the [`Reader`]'s latest "head" [`Commit`]
    ///
    /// This is the latest `Commit` the `Reader`'s can see.
    ///
    /// Calling [`push()`](Writer::push) would update the `Reader`'s head `Commit`.
    ///
    /// This is an shared "owned" `Commit` (it uses [`Arc`] internally).
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // Reader gets a reference.
    /// let reader: CommitRef<usize> = r.head();
    /// // Writer gets a reference.
    /// let writer: CommitRef<usize> = w.head_remote_ref();
    ///
    /// // Reader drops their reference.
    /// // Nothing happens, an atomic count is decremented.
    /// drop(reader);
    ///
    /// // Writer drops their reference.
    /// // They were the last reference, so they are
    /// // responsible for deallocating the backing data.
    /// drop(writer);
    /// ```
    pub fn head_remote_ref(&self) -> CommitRef<T> {
        Arc::clone(&self.remote)
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    /// All the `Patch`'s that **haven't** been [`commit()`](Writer::commit)'ed yet, aka, "staged" changes
    ///
    /// You are allowed to do anything to these `Patch`'s as they haven't
    /// been committed yet and the [`Writer`] does not necessarily need them.
    ///
    /// You can use something like `.staged().drain(..)` to get back all the `Patch`'s.
    ///
    /// All the `Patch`'s that have been [`commit()`](Writer::commit)'ed but not yet
    /// [`push()`](Writer::push)'ed are safely stored internally by the `Writer`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Add some changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///
    /// // We see and mutate the staged changes.
    /// assert_eq!(w.staged().len(), 1);
    ///
    /// // Let's actually remove that change.
    /// let removed = w.staged().remove(0);
    /// assert_eq!(w.staged().len(), 0);
    /// ```
    pub fn staged(&mut self) -> &mut Vec<Patch<T>> {
        &mut self.patches
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    /// All the `Patch`'s that **have** been [`commit()`](Writer::commit)'ed but not yet [`push()`](Writer::push)'ed
    ///
    /// You are not allowed to mutate these `Patch`'s as they haven't been
    /// [`push()`](Writer::push)'ed yet and the `Writer` may need them in the future.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Commit some changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// // We can see but not mutate functions.
    /// assert_eq!(w.committed_patches().len(), 1);
    /// ```
    pub const fn committed_patches(&self) -> &Vec<Patch<T>> {
        &self.patches_old
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// How many [`Reader`]'s are _currently_ accessing
    /// the current `Reader` head [`Commit`]?
    ///
    /// Note that this will always at least return `2`, as the
    /// [`Writer`] carries 2 strong references to the backing data `T`.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (_, mut w) = someday::new::<String>("".into());
    ///
    /// // The Writer, `w` holds 2 strong counts.
    /// assert_eq!(w.head_count().get(), 2);
    ///
    /// // Create and leak 8 Reader's.
    /// // Note however, the above Reader's
    /// // do not have strong references to the
    /// // underlying data, so they don't count.
    /// for i in 0..8 {
    ///     let reader = w.reader();
    ///     std::mem::forget(reader);
    /// }
    /// let r = w.reader();
    /// assert_eq!(w.head_count().get(), 2);
    ///
    /// // Leak the actual data 8 times.
    /// for i in 0..8 {
    ///     let head: CommitRef<String> = r.head();
    ///     std::mem::forget(head);
    /// }
    ///
    /// // Now there are 10 strong references.
    /// // (which will never be reclaimed since
    /// // we just leaked them)
    /// assert_eq!(w.head_count().get(), 10);
    /// ```
    pub fn head_count(&self) -> NonZeroUsize {
        let count = Arc::strong_count(&self.remote);
        assert!(count >= 2, "head_count() returned less than 2");

        // INVARIANT:
        // The fact that we have are passing an Arc
        // means this will always at-least output 1.
        NonZeroUsize::new(count).expect("head_count() returned 0")
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// How many [`Reader`]'s are there?
    ///
    /// Unlike [`Writer::head_count()`], this doesn't count references
    /// to the current data, it counts how many `Reader` objects are in existence.
    ///
    /// Note that this will always at least return `1`,
    /// as the [`Writer`] counts as a `Reader`.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // 2 Reader's (the Writer counts as a Reader).
    /// assert_eq!(w.reader_count().get(), 2);
    ///
    /// // Create and leak 8 Reader's.
    /// for i in 0..8 {
    ///     let reader = r.clone();
    ///     std::mem::forget(reader);
    /// }
    ///
    /// // Now there are 10.
    /// assert_eq!(w.reader_count().get(), 10);
    /// ```
    pub fn reader_count(&self) -> NonZeroUsize {
        let count = Arc::strong_count(&self.arc);

        // INVARIANT:
        // The fact that we have are passing an Arc
        // means this will always at-least output 1.
        NonZeroUsize::new(count).expect("head_count() returned 0")
    }

    /// Does a [`Reader`] object associated with this [`Writer`] exist?
    ///
    /// As noted in [`Writer::reader_count`], the `Writer` will always
    /// count as a `Reader`, meaning the strong count will always at
    /// least be `1`.
    ///
    /// If it is `1`, that means no `Reader` object exists,
    /// in that case, this function will return `false`.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // 1 `Reader` exists.
    /// assert!(w.readers_exist());
    ///
    /// // 0 `Reader`'s (excluding the `Writer`) exist.
    /// drop(r);
    /// assert!(!w.readers_exist());
    /// ```
    pub fn readers_exist(&self) -> bool {
        self.reader_count().get() > 1
    }

    /// Get the current status on the [`Writer`] and [`Reader`]
    ///
    /// This is a bag of various metadata about the current
    /// state of the `Writer` and `Reader`.
    ///
    /// If you only need 1 or a few of the fields in [`StatusInfo`],
    /// consider using their individual methods instead.
    pub fn status(&self) -> StatusInfo<'_, T> {
        StatusInfo {
            staged_patches: &self.patches,
            committed_patches: self.committed_patches(),
            head: self.head(),
            head_remote: self.head_remote(),
            head_count: self.head_count(),
            reader_count: self.reader_count(),
            timestamp: self.timestamp(),
            timestamp_remote: self.timestamp_remote(),
        }
    }
}
