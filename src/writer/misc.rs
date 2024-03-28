//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;

use crate::{
    commit::Commit, info::WriterInfo, patch::Patch, reader::Reader, writer::token::WriterToken,
    writer::Writer,
};

#[allow(unused_imports)] // docs
                         // use crate::Commit;

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    /// Same as [`crate::free::new`] but without creating a [`Reader`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new("hello");
    /// let w2 = Writer::new("hello");
    ///
    /// assert_eq!(w.data(), w2.data());
    /// assert_eq!(w.timestamp(), w2.timestamp());
    /// ```
    pub fn new(data: T) -> Self {
        crate::free::new_inner(Commit { data, timestamp: 0 })
    }

    #[inline]
    /// Replace all [`Writer::committed_patches`] with a simple clone operation.
    ///
    /// This will clear all [`Patch`]'s meant for syncing
    /// reclaimed data with [`Patch::CLONE`], which simply
    /// clones the [`Reader`]'s data into the [`Writer`].
    ///
    /// This could be used in the situation the `Patch`(s)
    /// are actually more expensive than just cloning.
    ///
    /// The committed `Patch`'s that were removed are returned.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Add _many_ `Patch`'s.
    /// for _ in 0..100_000 {
    ///     w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// }
    ///
    /// // Instead of re-executing all those functions,
    /// // cloning the data is probably faster.
    /// w.commit();
    /// w.just_clone();
    ///
    /// w.push();
    /// assert_eq!(w.data().len(), 100_000 * 3);
    /// assert_eq!(*w.data(), r.head().data);
    /// ```
    pub fn just_clone(&mut self) -> std::vec::Drain<'_, Patch<T>> {
        self.patches_old.push(Patch::CLONE);

        // Drain all but the Clone patch.
        self.patches_old.drain(..self.patches_old.len() - 1)
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    /// Restore all the staged changes.
    ///
    /// This removes all the `Patch`'s that haven't yet been [`commit()`](Writer::commit)'ed.
    ///
    /// Calling `Writer::staged().drain(..)` would be equivalent.
    ///
    /// Dropping the [`std::vec::Drain`] will drop the `Patch`'s.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Add some changes, but don't commit.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// assert_eq!(w.staged().len(), 1);
    ///
    /// // Restore changes.
    /// let drain = w.restore();
    /// assert_eq!(drain.count(), 1);
    /// ```
    pub fn restore(&mut self) -> std::vec::Drain<'_, Patch<T>> {
        self.patches.drain(..)
    }

    /// Shrinks the capacity of the `Patch` [`Vec`]'s as much as possible
    ///
    /// This calls [`Vec::shrink_to_fit()`] on the 2
    /// internal `Vec`'s in [`Writer`] holding:
    /// 1. The currently staged `Patch`'s
    /// 2. The already committed `Patch`'s
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (_, mut w) = someday::new::<String>("".into());
    ///
    /// // Capacity is 16.
    /// assert_eq!(w.committed_patches().capacity(), 16);
    /// assert_eq!(w.staged().capacity(),            16);
    ///
    /// // Commit 32 `Patch`'s
    /// for i in 0..32 {
    ///     w.add(Patch::Ptr(|w, _| *w = "".into()));
    ///     w.commit();
    /// }
    /// // Stage 16 `Patch`'s
    /// for i in 0..16 {
    ///     w.add(Patch::Ptr(|w, _| *w = "".into()));
    /// }
    ///
    /// // Commit capacity is now 32.
    /// assert_eq!(w.committed_patches().capacity(), 32);
    /// // This didn't change, we already had
    /// // enough space to store them.
    /// assert_eq!(w.staged().capacity(), 16);
    ///
    /// // Commit, push, shrink.
    /// w.commit();
    /// w.push();
    /// w.shrink_to_fit();
    ///
    /// // They're now empty and taking 0 space.
    /// assert_eq!(w.committed_patches().capacity(), 0);
    /// assert_eq!(w.staged().capacity(), 0);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        self.patches.shrink_to_fit();
        self.patches_old.shrink_to_fit();
    }

    /// Reserve capacity in the `Patch` [`Vec`]'s
    ///
    /// This calls [`Vec::reserve_exact()`] on the 2
    /// internal `Vec`'s in [`Writer`] holding:
    /// 1. The currently staged `Patch`'s
    /// 2. The already committed `Patch`'s
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (_, mut w) = someday::new::<String>("".into());
    ///
    /// // Capacity is 16.
    /// assert_eq!(w.committed_patches().capacity(), 16);
    /// assert_eq!(w.staged().capacity(),            16);
    ///
    /// // Reserve space for 48 more patches.
    /// w.reserve_exact(48);
    /// assert!(w.committed_patches().capacity() >= 48);
    /// assert!(w.staged().capacity()            >= 48);
    /// ```
    ///
    /// # Panics
    /// Panics if the new capacity exceeds [`isize::MAX`] bytes.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.patches.reserve_exact(additional);
        self.patches_old.reserve_exact(additional);
    }

    /// Is this [`Writer`] associated with this [`Reader`]?
    ///
    /// This returns `true` if both `self` and `other` are `Reader`'s from the same `Writer`.
    ///
    /// This means both `Reader`'s receive the same [`Commit`] upon calling [`Reader::head`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new(());
    ///
    /// // All `Reader`'s read from the same `Writer`.
    /// let r2 = w.reader();
    /// let r3 = r2.clone();
    /// assert!(w.connected(&r));
    /// assert!(w.connected(&r2));
    /// assert!(w.connected(&r3));
    ///
    /// // This one is completely separate.
    /// let (r4, _) = someday::new(());
    /// assert!(!r.connected(&r4));
    /// ```
    pub fn connected(&self, reader: &Reader<T>) -> bool {
        Arc::ptr_eq(&self.arc, &reader.arc)
    }

    /// Disconnect from the [`Reader`]'s associated with this [`Writer`].
    ///
    /// This completely severs the link between the
    /// `Reader`'s associated with this `Writer`.
    ///
    /// Any older `Reader`'s will no longer receive [`Commit`]'s
    /// from this `Writer`, and [`Reader::writer_dropped`] will start
    /// to return `true`. From the perspective of the older `Reader`'s,
    /// calling this function is the same as this `Writer` being dropped.
    ///
    /// Any future `Reader`'s created after this function
    /// are completely separate from the past `Reader`'s.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new("");
    ///
    /// // Connected `Reader` <-> `Writer`.
    /// assert!(w.connected(&r));
    ///
    /// // Now, disconnected.
    /// w.disconnect();
    /// assert!(!w.connected(&r));
    ///
    /// // The older `Reader` won't see pushes anymore.
    /// w.add_commit_push(|w, _| *w = "hello");
    /// assert_eq!(*w.data(), "hello");
    /// assert_eq!(r.head().data, "");
    ///
    /// // But, newer `Reader`'s will.
    /// let r2 = w.reader();
    /// assert_eq!(r2.head().data, "hello");
    /// ```
    pub fn disconnect(&mut self) {
        self.token = WriterToken::new();
        self.arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&self.remote)));
    }

    #[allow(clippy::missing_panics_doc, clippy::type_complexity)]
    /// Consume this [`Writer`] and return the inner components.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Commit some changes.
    /// w.add(Patch::Ptr(|w, _| w.push_str("a")));
    /// w.commit();
    ///
    /// // Add but don't commit.
    /// w.add(Patch::Ptr(|w, _| w.push_str("b")));
    ///
    /// let WriterInfo {
    ///     writer,
    ///     reader,
    ///     staged,
    ///     committed_patches,
    /// } = w.into_inner();
    ///
    /// assert_eq!(writer.data, "a");
    /// assert_eq!(reader.data, ""); // We never `push()`'ed, so Readers saw nothing.
    /// assert_eq!(staged.len(), 1);
    /// assert_eq!(committed_patches.len(), 1);
    /// ```
    pub fn into_inner(self) -> WriterInfo<T> {
        WriterInfo {
            // INVARIANT: local must be initialized after push()
            writer: self.local.unwrap(),
            reader: self.remote,
            staged: self.patches,
            committed_patches: self.patches_old,
        }
    }
}
