//! `Reader<T>`

//---------------------------------------------------------------------------------------------------- Use
use crate::{
    commit::{Commit, CommitRef},
    free::INIT_VEC_CAP,
    writer::{WriterReviveToken, WriterToken},
    Writer,
};
use std::{num::NonZeroUsize, sync::Arc};

//---------------------------------------------------------------------------------------------------- Reader
/// Reader(s) who can read some data `T`.
///
/// [`Reader`]'s can cheaply [`Clone`] themselves and there
/// is no limit to how many there can be.
///
/// `Reader`'s can cheaply acquire access to the latest data
/// that the [`Writer`] has [`push()`](Writer::push)'ed by using [`Reader::head()`].
///
/// This access:
/// - Is wait-free and sometimes lock-free
/// - Will never block the `Writer`
/// - Will gain a [`CommitRef`] of the data `T`
///
/// ## Usage
/// This example covers the typical usage of a `Reader`:
/// - Creating some other `Reader`'s
/// - Acquiring the latest head [`Commit`] of data
/// - Viewing the data, the timestamp
/// - Hanging onto that data for a while
/// - Repeat
///
/// ```rust
/// # use someday::*;
/// // Create a Reader/Writer pair of a `String`.
/// let (reader, writer) = someday::new("".into());
///
/// // To clarify the types of these things:
/// // This is the Reader.
/// // It can clone itself infinite amount of
/// // time very cheaply.
/// let reader: Reader<String> = reader;
/// for _ in 0..100 {
///     // Pretty cheap operation.
///     let another_reader = reader.clone();
///     // We can send Reader's to other threads.
///     std::thread::spawn(move || assert_eq!(another_reader.head().data, ""));
/// }
///
/// // This is the single Writer, it cannot clone itself.
/// let mut writer: Writer<String> = writer;
///
/// // Both Reader and Writer are at timestamp 0 and see no changes.
/// assert_eq!(writer.timestamp(), 0);
/// assert_eq!(reader.head().timestamp, 0);
/// assert_eq!(*writer.data(), "");
/// assert_eq!(reader.head().data, "");
///
/// // Move the Writer into another thread
/// // and make it do some work in the background.
/// std::thread::spawn(move || {
///     // 1. Append to string
///     // 2. Commit it
///     // 3. Push so that Readers can see
///     // 4. Repeat
///     //
///     // This is looping at an extremely fast rate
///     // and real code probably wouldn't do this, although
///     // just for the example...
///     loop {
///         writer.add(Patch::Ptr(|w, _| w.push_str("abc")));
///         writer.commit();
///         writer.push();
///     }
/// });
/// # std::thread::sleep(std::time::Duration::from_secs(1));
///
/// // Even though the Writer _just_ started
/// // the shared string is probably already
/// // pretty long at this point.
/// let head_commit: CommitRef<String> = reader.head();
/// // Wow, longer than 5,000 bytes!
/// assert!(head_commit.data.len() > 5_000);
///
/// // The timestamp is probably pretty high already too.
/// assert!(head_commit.timestamp > 500);
///
/// // We can continually call `.head()` and keep
/// // retrieving the latest data. Doing this
/// // will _not_ block the Writer from continuing.
/// let mut last_head: CommitRef<String> = reader.head();
/// let mut new_head:  CommitRef<String> = reader.head();
/// for _ in 0..10 {
///     last_head = reader.head();
///
///     // Wait just a little...
///     std::thread::sleep(std::time::Duration::from_millis(10));
///     # // CI makes this non-reliable, add more sleep time.
///     # std::thread::sleep(std::time::Duration::from_millis(90));
///
///     new_head = reader.head();
///
///     // We got new data!
///     assert!(last_head != new_head);
///     assert!(last_head.timestamp < new_head.timestamp);
/// }
///
/// // We can hold onto these `CommitRef`'s _forever_
/// // although it means we will be using more memory.
/// let head_commit: CommitRef<String> = reader.head();
///
/// // If we're the last ones holding onto this `Commit`
/// // we'll be the ones running the `String` drop code here.
/// drop(head_commit);
/// ```
#[derive(Clone, Debug)]
pub struct Reader<T: Clone> {
    /// The atomic pointer to the `Arc` that all readers enter through.
    ///
    /// This is `swap()` updated by the `Writer`.
    pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<Commit<T>>>>,
    /// Has the associated `Writer` to this `Reader` been dropped?
    pub(super) token: WriterToken,
    /// Optional cache of the latest `head()`.
    pub(super) cache: Option<Arc<Commit<T>>>,
}

impl<T: Clone> Reader<T> {
    #[inline]
    #[must_use]
    /// Acquire the latest [`CommitRef`] pushed by the [`Writer`].
    ///
    /// This function will never block.
    ///
    /// This will retrieve the latest data the [`Writer`] is willing
    /// to share with [`Writer::push()`].
    ///
    /// After [`Writer::push()`] finishes, it is atomically
    /// guaranteed that [`Reader`]'s who then call [`Reader::head()`]
    /// will see those new changes.
    ///
    /// ```rust
    /// # use someday::*;
    /// // Create a Reader/Writer pair.
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// // Both Reader and Writer are at timestamp 0 and see no changes.
    /// assert_eq!(w.timestamp(), 0);
    /// assert_eq!(r.head().timestamp, 0);
    /// assert_eq!(w.data(), "");
    /// assert_eq!(r.head().data, "");
    ///
    /// // Writer commits some changes locally.
    /// w.add(Patch::Ptr(|w, _| *w = "hello".into()));
    /// w.commit();
    ///
    /// // Writer sees local changes.
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.data(), "hello");
    ///
    /// // Reader does not, because Writer did not `push()`.
    /// let head: CommitRef<String> = r.head();
    /// assert_eq!(head.timestamp, 0);
    /// assert_eq!(head.data, "");
    ///
    /// // Writer pushs to the Readers.
    /// w.push();
    ///
    /// // Now Readers see changes.
    /// let head: CommitRef<String> = r.head();
    /// assert_eq!(head.timestamp, 1);
    /// assert_eq!(head.data, "hello");
    /// ```
    pub fn head(&self) -> CommitRef<T> {
        self.arc.load_full()
    }

    /// Cache a [`Commit`] and return it.
    ///
    /// Upon first cache or cache after [`Reader::cache_take`], this function
    /// will call [`Reader::head`] and store it internally for quick access.
    ///
    /// Subsequent calls to [`Reader::cache`] will return the
    /// _same_ [`Commit`] forever, and never update.
    ///
    /// # Memory usage
    /// Be aware that this causes the [`Reader`] to hold onto a [`CommitRef`].
    /// As such, the `CommitRef` will not be dropped until the cache is cleared
    /// or this [`Reader`] is dropped.
    ///
    /// # Example
    /// ```rust
    /// # use someday::*;
    /// let (mut r, mut w) = someday::new(());
    ///
    /// // Our first cache access, this will call
    /// // `Reader::head()` and save it internally.
    /// let cache: CommitRef<()> = r.cache();
    /// assert_eq!(cache.timestamp, 0);
    /// assert!(r.cache_up_to_date());
    ///
    /// // But... the `Writer` continues to push.
    /// w.add_commit_push(|_, _| {});
    ///
    /// // Now our cache is technically out-of-date.
    /// assert!(!r.cache_up_to_date());
    /// // Future calls will return the out-of-date cache.
    /// assert_eq!(r.cache().timestamp, 0);
    /// ```
    pub fn cache(&mut self) -> CommitRef<T> {
        if let Some(cache) = self.cache.as_ref() {
            Arc::clone(cache)
        } else {
            // Else, update the cached commit and return it.
            let head = self.head();
            self.cache = Some(Arc::clone(&head));
            head
        }
    }

    /// Cache a [`Commit`], updating it if needed, and return it.
    ///
    /// This is the same as [`Reader::cache`] except it this function
    /// will update the internal cache such that it _always_ returns
    /// the latest [`Reader::head`].
    ///
    /// If the cache is already the same, this is a much
    /// cheaper access to the `Commit` than [`Reader::head`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (mut r, mut w) = someday::new(());
    ///
    /// // Our first cache access, this will call
    /// // `Reader::head()` and save it internally.
    /// let cache: CommitRef<()> = r.cache_update();
    /// assert_eq!(cache.timestamp, 0);
    /// assert!(r.cache_up_to_date());
    ///
    /// // The `Writer` continues to push.
    /// w.add_commit_push(|_, _| {});
    ///
    /// // Using `cache_update()`, our cache always is up-to-date.
    /// let cache: CommitRef<()> = r.cache_update();
    /// assert_eq!(cache.timestamp, 1);
    /// assert!(r.cache_up_to_date());
    /// ```
    pub fn cache_update(&mut self) -> CommitRef<T> {
        if !self.cache_up_to_date() {
            self.cache = Some(self.head());
        }

        self.cache()
    }

    #[must_use]
    /// Is the [`Reader::cache`] up to date?
    ///
    /// This returns `true` if [`Reader::cache`] and [`Reader::head`]
    /// were to return the same [`CommitRef`].
    ///
    /// If [`Reader::cache`] was never called (or [`Reader::cache_take`]'n),
    /// then this function returns `false.`
    ///
    /// ```rust
    /// # use someday::*;
    /// let (mut r, mut w) = someday::new(());
    ///
    /// // There is no cache, this returns `false`.
    /// assert!(!r.cache_up_to_date());
    ///
    /// // Set cache.
    /// r.cache();
    /// assert!(r.cache_up_to_date());
    ///
    /// // The `Writer` pushes.
    /// w.add_commit_push(|_, _| {});
    ///
    /// // Cache is now out-of-date.
    /// assert!(!r.cache_up_to_date());
    ///
    /// // Clear the cache.
    /// r.cache_take();
    /// assert!(!r.cache_up_to_date());
    /// ```
    pub fn cache_up_to_date(&self) -> bool {
        self.cache.as_ref().is_some_and(|cache| {
            let head = self.arc.load();
            Arc::ptr_eq(&head, cache)
        })
    }

    /// Take the cache out of the `Reader`.
    ///
    /// This returns the internal [`CommitRef`] created by
    /// [`Reader::cache`] and [`Reader::cache_update`].
    ///
    /// This returns `None` if the cache was
    /// never created or taken in the past.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (mut r, mut w) = someday::new(());
    ///
    /// // Set cache...
    /// r.cache();
    /// assert!(r.cache_up_to_date());
    ///
    /// // ...and take it.
    /// let cache: CommitRef<()> = r.cache_take().unwrap();
    /// assert!(!r.cache_up_to_date());
    /// assert_eq!(cache.timestamp, 0);
    /// ```
    pub fn cache_take(&mut self) -> Option<CommitRef<T>> {
        self.cache.take()
    }

    #[must_use]
    /// Borrow the internal cache, whether initialized or not.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (mut r, mut w) = someday::new(());
    ///
    /// // No cache, returns None.
    /// assert!(r.cache_as_ref().is_none());
    ///
    /// // Set cache, and borrow it.
    /// r.cache();
    /// assert!(r.cache_as_ref().is_some());
    /// ```
    pub const fn cache_as_ref(&self) -> Option<&CommitRef<T>> {
        self.cache.as_ref()
    }

    #[inline]
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    /// How many [`Reader`]'s are there?
    ///
    /// This is the same as [`Writer::reader_count()`].
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new(());
    ///
    /// // `w` + `r` == 2 (Writer's count as a Reader).
    /// assert_eq!(w.reader_count().get(), 2);
    /// assert_eq!(r.reader_count().get(), 2);
    ///
    /// let r3 = w.reader();
    ///
    /// assert_eq!(w.reader_count().get(), 3);
    /// assert_eq!(r.reader_count().get(), 3);
    /// ```
    pub fn reader_count(&self) -> NonZeroUsize {
        let count = Arc::strong_count(&self.arc);

        // INVARIANT:
        // The fact that we have are passing an Arc
        // means this will always at-least output 1.
        NonZeroUsize::new(count).expect("reader_count() returned 0")
    }

    #[must_use]
    /// This returns whether the associated [`Writer`] to this
    /// [`Reader`] has been dropped (or [`Writer::disconnect`]'ed).
    ///
    /// Note that even if this returns `true`, [`Reader::try_into_writer`]
    /// is not guaranteed to succeed as other `Reader`'s could race towards
    /// becoming the new `Writer`.
    ///
    /// It is guaranteed _one_ of them will succeed, but not necessarily _this_ `Reader`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new(());
    /// assert_eq!(r.writer_dropped(), false);
    ///
    /// drop(w);
    /// assert_eq!(r.writer_dropped(), true);
    /// ```
    pub fn writer_dropped(&self) -> bool {
        self.token.is_dead()
    }

    #[must_use]
    /// Are both these [`Reader`]'s associated with the same [`Writer`]?
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
    /// assert!(r.connected(&r2));
    /// assert!(r.connected(&r3));
    ///
    /// // This one is completely separate.
    /// let (r4, _) = someday::new(());
    /// assert!(!r.connected(&r4));
    /// ```
    pub fn connected(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.arc, &other.arc)
    }

    #[must_use]
    /// Is this [`Reader`] associated with this [`Writer`]?
    ///
    /// This returns `true` if `self` is associated with the passed `writer`.
    ///
    /// This means `self` receives the [`Commit`]'s that `writer` pushes.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new(());
    ///
    /// // Connected `Reader` <-> `Writer`.
    /// assert!(r.connected_writer(&w));
    ///
    /// // This one is completely separate.
    /// let (_, w2) = someday::new(());
    /// assert!(!r.connected_writer(&w2));
    /// ```
    pub fn connected_writer(&self, writer: &Writer<T>) -> bool {
        Arc::ptr_eq(&self.arc, &writer.arc)
    }

    /// Attempt to transform this [`Reader`] into an associated [`Writer`].
    ///
    /// If the original `Writer` associated with this `Reader` is gone,
    /// this function will turn `self` into a new `Writer`, while maintaining
    /// the connection with any other `Reader`'s.
    ///
    /// Any future [`Commit`] pushed by the returned `Writer`
    /// will be observed by other `Reader`'s.
    ///
    /// # Errors
    /// This returns back `Err(self)` if either:
    /// 1. The associated `Writer` is still alive
    /// 2. Another `Reader` is currently in this function, becoming the `Writer`
    ///
    /// # Example
    /// ```rust
    /// # use someday::*;
    /// let (r, w) = someday::new(String::from("hello"));
    ///
    /// // A secondary `Reader`, forget about this for now.
    /// let r2 = r.clone();
    ///
    /// // The `Writer` is still alive... this will fail.
    /// let r: Reader<String> = match r.try_into_writer() {
    ///     Ok(_) => panic!("this can never happen"),
    ///     Err(r) => r,
    /// };
    ///
    /// // The `Writer` is now dropped, one of the
    /// // `Reader`'s can now be "promoted".
    /// drop(w);
    /// assert!(r.writer_dropped());
    /// let mut new_writer: Writer<String> = r.try_into_writer().unwrap();
    ///
    /// // This new `Writer` is _still_ connected
    /// // to the previous `Reader`'s...!
    /// new_writer.add_commit_push(|w, _| {
    ///     w.push_str(" world!");
    /// });
    ///
    /// // The previous `Reader` sees the push!
    /// assert_eq!(r2.head().data, "hello world!");
    /// ```
    pub fn try_into_writer(self) -> Result<Writer<T>, Self> {
        let Some(writer_revive_token) = self.token.try_revive() else {
            return Err(self);
        };

        //------------------------------------------------------------
        // Past this point, we:
        // 1. Are the only `Reader` here
        // 2. Can safely turn into a `Writer` since it was dropped
        //------------------------------------------------------------

        let remote = self.head();
        let local = Some(remote.as_ref().clone());
        let arc = self.arc;
        let patches = Vec::with_capacity(INIT_VEC_CAP);
        let patches_old = Vec::with_capacity(INIT_VEC_CAP);

        // INVARIANT: We must tell the token that we have successfully revived the `Writer`.
        WriterReviveToken::revived(writer_revive_token);

        let writer = Writer {
            token: self.token,
            local,
            remote,
            arc,
            patches,
            patches_old,
        };

        Ok(writer)
    }

    #[must_use]
    /// Fork off from the current [`Reader::head`] [`Commit`] and create a [`Writer`].
    ///
    /// This function is identical [`Writer::fork`], although the
    /// `Reader`'s most recent `Commit` will be used as the base instead.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::new());
    ///
    /// // Connected `Reader` <-> `Writer`.
    /// assert!(r.connected_writer(&w));
    ///
    /// // Add local changes, but don't push.
    /// w.add_commit(|s, _| {
    ///     s.push_str("hello");
    /// });
    /// assert_eq!(w.data(), "hello");
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(r.head().data, "");
    /// assert_eq!(r.head().timestamp, 0);
    ///
    /// // Fork the _Reader_ off into another `Writer`.
    /// let mut w2 = r.fork();
    ///
    /// // It inherits the data of the `Reader`.
    /// assert_eq!(w2.data(), "");
    /// assert_eq!(w2.timestamp(), 0);
    ///
    /// // And has no relation to the previous `Writer/Reader`'s.
    /// assert!(!w2.connected(&r));
    /// ```
    pub fn fork(&self) -> Writer<T> {
        let remote = self.head();
        let local = remote.as_ref().clone();
        let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));

        Writer {
            token: WriterToken::new(),
            local: Some(local),
            remote,
            arc,
            patches: Vec::with_capacity(INIT_VEC_CAP),
            patches_old: Vec::with_capacity(INIT_VEC_CAP),
        }
    }
}

//---------------------------------------------------------------------------------------------------- Trait Impl
impl<T: Clone> From<&Writer<T>> for Reader<T> {
    #[inline]
    fn from(value: &Writer<T>) -> Self {
        value.reader()
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Reader<T>
where
    T: Clone + serde::Serialize,
{
    #[inline]
    /// This will call `head()`, then serialize the resulting [`CommitRef`].
    ///
    /// ```rust
    /// # use someday::*;
    ///
    /// let (r, _) = someday::new(String::from("hello"));
    ///
    /// let json = serde_json::to_string(&r).unwrap();
    /// assert_eq!(json, "{\"timestamp\":0,\"data\":\"hello\"}");
    /// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CommitRef::serialize(&self.head(), serializer)
    }
}

#[cfg(feature = "bincode")]
impl<T> bincode::Encode for Reader<T>
where
    T: Clone + bincode::Encode,
{
    #[inline]
    /// This will call `head()`, then serialize the resulting [`CommitRef`].
    ///
    /// ```rust
    /// # use someday::*;
    ///
    /// let (r, _) = someday::new(String::from("hello"));
    /// let config = bincode::config::standard();
    ///
    /// let encoded = bincode::encode_to_vec(&r, config).unwrap();
    /// let decoded: Commit<String> = bincode::decode_from_slice(&encoded, config).unwrap().0;
    /// assert_eq!(decoded, Commit { timestamp: 0, data: String::from("hello") });
    /// ```
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        CommitRef::encode(&self.head(), encoder)
    }
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshSerialize for Reader<T>
where
    T: Clone + borsh::BorshSerialize,
{
    #[inline]
    /// This will call `self.head().data`, then serialize your `T`.
    ///
    /// ```rust
    /// # use someday::*;
    ///
    /// let (r, _) = someday::new(String::from("hello"));
    ///
    /// let encoded = borsh::to_vec(&r).unwrap();
    /// let decoded: Commit<String> = borsh::from_slice(&encoded).unwrap();
    /// assert_eq!(decoded, Commit { timestamp: 0, data: String::from("hello") });
    /// ```
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        CommitRef::serialize(&self.head(), writer)
    }
}
