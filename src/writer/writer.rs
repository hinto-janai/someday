//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{borrow::Borrow, sync::Arc};

use crate::{
    commit::{Commit, CommitRef},
    patch::Patch,
    reader::Reader,
    writer::WriterToken,
};

#[allow(unused_imports)] // docs
use std::sync::Mutex;

//---------------------------------------------------------------------------------------------------- Writer
/// The single [`Writer`] of some data `T`.
///
/// The `Writer`:
/// 1. Stores your [`Patch`]'s with [`add()`](Writer::add)
/// 2. Actually applies them to `T` by [`commit()`](Writer::commit)'ing
/// 3. Can [`push()`](Writer::push) so that [`Reader`]'s can see the changes
///
/// The `Writer` can also generate infinite `Reader`'s with [`Writer::reader()`].
///
/// ## Invariants
/// Some invariants that the `Writer` upholds, that you can rely on:
///
/// | Invariant           | Description |
/// |---------------------|-------------|
/// | No "rebasing" | [`Writer::timestamp()`] will always be greater than or equal to the [`Reader::head()`]'s timestamp.
/// | [`PartialEq`] | If the [`Writer::timestamp()`] is the same as the [`Reader::head()`]'s timestamp, the data `T` is the same as well (as long as [`Patch`]'s are deterministic). This can serve as a very cheap way to compare data (just compare the timestamps).
/// | 1 Writer            | There can only ever be 1 `Writer` at any given moment (at least, without shared mutual exclusion like [`Arc`] + [`Mutex`]).
/// | Poison              | If a `Writer` panics mid-[`push()`](Writer::push), the data can only be poisoned on the `Writer` side - i.e. `Reader`'s will be completely fine if the `Writer` panics, other `Writer`'s (e.g [`Arc<Mutex<Writer<T>>>`]) _may_ panic as well on any function that accesses `T`.
///
/// ## Usage
/// This example covers the typical usage of a `Writer`:
/// - Creating some `Reader`'s
/// - Adding some `Patch`'s
/// - Viewing the staged `Patch`'s, modifying them
/// - Committing those changes
/// - Pushing those changes to the `Reader`'s
///
/// ```rust
/// use someday::{*,info::*};
///
/// // Create a Reader/Writer pair that can "apply"
/// // the `PatchString` patch to `String`'s.
/// let (r, w) = someday::new("".into());
///
/// // To clarify the types of these things:
/// // This is the Reader.
/// // It can clone itself an infinite
/// // amount of time very cheaply.
/// let r: Reader<String> = r;
/// for _ in 0..10_000 {
///     let another_reader = r.clone(); // akin to Arc::clone()
/// }
///
/// // This is the single Writer, it cannot clone itself.
/// let mut w: Writer<String> = w;
///
/// // Both Reader and Writer are at timestamp 0 and see no changes.
/// assert_eq!(w.timestamp(), 0);
/// assert_eq!(r.head().timestamp, 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head().data, "");
///
/// // The Writer can add many `Patch`'s
/// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
/// w.add(Patch::Ptr(|w, _| w.push_str("def")));
/// w.add(Patch::Ptr(|w, _| w.push_str("ghi")));
/// w.add(Patch::Ptr(|w, _| w.push_str("jkl")));
///
/// // But `add()`'ing does not actually modify the
/// // local (Writer) or remote (Readers) data, it
/// // just "stages" them.
/// assert_eq!(w.timestamp(), 0);
/// assert_eq!(r.head().timestamp, 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head().data, "");
///
/// // We can see our "staged" patches here.
/// let staged = w.staged();
/// assert_eq!(staged.len(), 4);
///
/// // Let's actually remove a patch.
/// staged.remove(3); // w.push_str("jkl")
///
/// // Okay, now let's commit locally.
/// let commit_info: CommitInfo = w.commit();
/// // We applied 3 patches in total.
/// assert_eq!(commit_info.patches, 3);
/// // And added 1 commit (timestamp).
/// assert_eq!(w.timestamp(), 1);
///
/// // We haven't pushed yet, so the Readers
/// // are still un-aware of our local changes.
/// assert_eq!(w.timestamp(), 1);
/// assert_eq!(r.head().timestamp, 0);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head().data, "");
///
/// // Now we push.
/// let push_info: PushInfo = w.push();
/// // We pushed 1 commit in total.
/// assert_eq!(push_info.commits, 1);
/// // Our staged functions are now gone.
/// assert_eq!(w.staged().len(), 0);
///
/// // The Readers are now in sync.
/// assert_eq!(w.timestamp(), 1);
/// assert_eq!(r.head().timestamp, 1);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head().data, "abcdefghi");
/// ```
pub struct Writer<T: Clone> {
    /// Only set to `false` when we are `drop()`'ed.
    pub(crate) token: WriterToken,

    /// The writer's local mutually
    /// exclusive copy of the data.
    ///
    /// This is an `Option` only because there's
    /// a brief moment in `push()` where we need
    /// to send off `local`, but we can't yet swap it
    /// with the old data.
    ///
    /// It will be `None` in-between those moments and
    /// the invariant is that is MUST be `Some` before
    /// `push()` is over.
    ///
    /// This _could_ be a `MaybeUninit` instead, although:
    /// 1. Requires `unsafe`
    /// 2. Is actually unsafe if we panic mid-`push()`
    ///
    /// In the case code panics _right_ after we set this
    /// to `None` and before we set it back to `Some`, it
    /// will be in an uninitialized state.
    ///
    /// Thankfully it's an `Option`, and we `.unwrap()` on
    /// each access, if it were a `MaybeUninit`, UB.
    pub(crate) local: Option<Commit<T>>,

    /// The current data the remote `Reader`'s can see.
    pub(crate) remote: CommitRef<T>,

    /// The AtomicPtr that `Reader`'s enter through.
    /// Calling `.load()` would load the `remote` above.
    pub(crate) arc: Arc<arc_swap::ArcSwap<Commit<T>>>,

    /// Patches that have not yet been applied.
    pub(crate) patches: Vec<Patch<T>>,

    /// Patches that were already applied,
    /// that must be re-applied to the old `T`.
    pub(crate) patches_old: Vec<Patch<T>>,
}

//---------------------------------------------------------------------------------------------------- Private writer functions
impl<T: Clone> Writer<T> {
    #[allow(clippy::option_if_let_else, clippy::inline_always)]
    #[inline(always)]
    /// Borrow `self.local`.
    pub(crate) const fn local_as_ref(&self) -> &Commit<T> {
        // INVARIANT: `local` must be initialized after push()
        match self.local.as_ref() {
            Some(local) => local,
            None => panic!("the `Writer`'s local data <T> was not initialized (poisoned)"),
        }
    }

    #[allow(clippy::option_if_let_else, clippy::inline_always)]
    #[inline(always)]
    /// Borrow `self.local`.
    pub(crate) fn local_as_mut(&mut self) -> &mut Commit<T> {
        // INVARIANT: `local` must be initialized after push()
        match self.local.as_mut() {
            Some(local) => local,
            None => panic!("the `Writer`'s local data <T> was not initialized (poisoned)"),
        }
    }
}

//---------------------------------------------------------------------------------------------------- Writer trait impl
impl<T> std::fmt::Debug for Writer<T>
where
    T: Clone + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("local", &self.local)
            .field("remote", &self.remote)
            .field("arc", &self.arc)
            .finish_non_exhaustive()
    }
}

impl<T: Clone> From<T> for Writer<T> {
    /// Same as [`crate::free::new`] but without creating a [`Reader`].
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T: Clone> From<Commit<T>> for Writer<T> {
    /// Same as [`crate::free::from_commit`] but without creating a [`Reader`].
    fn from(commit: Commit<T>) -> Self {
        crate::free::new_inner(commit)
    }
}

impl<T: Clone> From<CommitRef<T>> for Writer<T> {
    /// Same as [`crate::free::from_commit`] but without creating a [`Reader`].
    fn from(commit: CommitRef<T>) -> Self {
        crate::free::new_inner(commit.as_ref().clone())
    }
}

impl<T> Default for Writer<T>
where
    T: Clone + Default,
{
    /// Only generates the [`Writer`].
    ///
    /// This initializes your data `T` with [`Default::default()`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (_, w1) = someday::new::<usize>(Default::default());
    /// let w2      = Writer::<usize>::default();
    ///
    /// assert_eq!(*w1.data(), 0);
    /// assert_eq!(*w2.data(), 0);
    /// ```
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> std::ops::Deref for Writer<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.local_as_ref().data
    }
}

impl<T: Clone> Borrow<T> for Writer<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.local_as_ref().data
    }
}

impl<T: Clone> AsRef<T> for Writer<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        &self.local_as_ref().data
    }
}

impl<T: Clone> TryFrom<Reader<T>> for Writer<T> {
    type Error = Reader<T>;

    /// Calls [`Reader::try_into_writer`].
    fn try_from(reader: Reader<T>) -> Result<Self, Self::Error> {
        Reader::try_into_writer(reader)
    }
}

impl<T: Clone> Clone for Writer<T> {
    /// This is the exact same as [`Writer::fork`].
    ///
    /// Note that this means cloning a [`Writer`] completely
    /// disconnects it from previous [`Reader`]'s.
    ///
    /// This does _not_ create 2 `Writer`'s to the same data,
    /// as that is not allowed.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::sync::*;
    /// let (r, mut w) = someday::new(String::new());
    ///
    /// // The clone has no relation to the previous `Writer/Reader`'s.
    /// let clone: Writer<String> = w.clone();
    /// assert!(!clone.connected(&r));
    ///
    /// // Wrapping `Writer` in a shared mutual exclusion primitive
    /// // allows it to be cheaply cloned, without `fork()`-like behavior.
    /// let shared = Arc::new(Mutex::new(clone));
    /// let reader = shared.lock().unwrap().reader();
    /// assert!(shared.lock().unwrap().connected(&reader));
    /// assert!(!shared.lock().unwrap().connected(&r));
    /// ```
    fn clone(&self) -> Self {
        self.fork()
    }
}
