//! `Reader<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
	sync::{
		Arc,
		atomic::{
			AtomicBool,
			Ordering,
		},
	},
	time::Duration,
	num::NonZeroUsize,
};
use crate::{
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	Writer,
};

//---------------------------------------------------------------------------------------------------- Reader
/// Reader(s) who can atomically read some data `T`.
///
/// [`Reader`]'s can cheaply [`Clone`] themselves and there
/// is no limit to how many there can be.
///
/// [`Reader`]'s can cheaply acquire access to the latest data
/// that the [`Writer`] has [`push()`](Writer::push)'ed by using [`Reader::head()`].
///
/// This access:
/// - Is wait-free and sometimes lock-free
/// - Will never block the [`Writer`]
/// - Will gain a shared owned [`Commit`] of the data `T`
///
/// ## Commits
/// The main object [`CommitRef`], returned from the main function [`Reader::head()`] is more or less:
/// ```rust
/// struct CommitRef<T> {
///     timestamp: usize,
///     data: std::sync::Arc<T>,
/// }
/// ```
/// so as long as that [`CommitRef`] is alive, the data will stay alive.
///
/// These [`CommitRef`]'s are cheaply clonable and sharable with other threads.
///
/// ## Usage
/// This example covers the typical usage of a `Reader`:
/// - Creating some other [`Reader`]'s
/// - Acquiring the latest head [`Commit`] of data
/// - Viewing the data, the timestamp
/// - Hanging onto that data for a while
/// - Repeat
///
/// ```rust
/// use someday::{Writer,Reader,Commit,CommitOwned,CommitRef};
///
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
///     std::thread::spawn(move || assert_eq!(another_reader.head().data(), ""));
/// }
///
/// // This is the single Writer, it cannot clone itself.
/// let mut writer: Writer<String> = writer;
///
/// // Both Reader and Writer are at timestamp 0 and see no changes.
/// assert_eq!(writer.timestamp(), 0);
/// assert_eq!(reader.timestamp(), 0);
/// assert_eq!(*writer.data(), "");
/// assert_eq!(reader.head().data(), "");
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
///         writer.add(|w, _| w.push_str("abc"));
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
/// assert!(head_commit.data().len() > 5_000);
///
/// // The timestamp is probably pretty high already too.
/// assert!(head_commit.timestamp() > 500);
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
///     assert!(last_head.timestamp() < new_head.timestamp());
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
#[derive(Clone,Debug)]
pub struct Reader<T: Clone> {
	/// The atomic pointer to the `Arc` that all readers enter through.
	///
	/// This is `swap()` updated by the `Writer`.
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<CommitOwned<T>>>>,
	/// Is the `Writer` currently in the process of swapping the above pointer?
	pub(super) swapping: Arc<AtomicBool>,
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
	/// assert_eq!(r.timestamp(), 0);
	/// assert_eq!(w.data(), "");
	/// assert_eq!(r.head().data(), "");
	///
	/// // Writer commits some changes locally.
	/// w.add(|w, _| *w = "hello".into());
	/// w.commit();
	///
	/// // Writer sees local changes.
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(w.data(), "hello");
	///
	/// // Reader does not, because Writer did not `push()`.
	/// let head: CommitRef<String> = r.head();
	/// assert_eq!(head.timestamp(), 0);
	/// assert_eq!(head.data(), "");
	///
	/// // Writer pushs to the Readers.
	/// w.push();
	///
	/// // Now Readers see changes.
	/// let head: CommitRef<String> = r.head();
	/// assert_eq!(head.timestamp(), 1);
	/// assert_eq!(head.data(), "hello");
	/// ```
	pub fn head(&self) -> CommitRef<T> {
		// May be slower for readers,
		// although, more maybe better
		// to prevent writer starvation.
		// let arc = self.arc.load_full();

		// Faster for readers.
		// May cause writer starvation
		// (writer will clone all the
		// time because there are always
		// strong arc references).
		arc_swap::Guard::into_inner(self.arc.load())
	}

	#[inline]
	#[must_use]
	/// Acquire the latest [`CommitRef`] pushed by the [`Writer`], but wait a little to cooperate.
	///
	/// This is the same as [`Reader::head()`] but if the [`Writer`] is currently
	/// trying to push new data, this function will wait for `duration` amount
	/// of time before forcefully acquiring the latest [`CommitRef`] anyway.
	///
	/// Realistically, `duration` can be a small number as
	/// the time it takes [`Writer`] to "push" new data is very small.
	/// (1 new `Arc` and an atomic pointer swap).
	///
	/// `std::time::Duration::from_millis(1)` will most likely be more
	/// than enough time for the [`Writer`] to finish (depending on how
	/// long it takes [`Writer`] to re-apply all your `Patch`'s).
	///
	/// This will forcefully call [`head()`](Reader::head) after
	/// sleeping regardless if the [`Writer`] is pushing.
	///
	/// Consider using [`Reader::head_do()`] or [`Reader::head_spin()`]
	/// instead of paying the price of sleeping a thread for a tiny duration.
	pub fn head_wait(&self, duration: Duration) -> CommitRef<T> {
		// Writer is not swapping, acquire head commit.
		if !self.swapping() {
			return self.head();
		}

		// Else sleep and acquire.
		std::thread::sleep(duration);
		self.head()
	}

	#[inline]
	#[must_use]
	/// Acquire the latest [`CommitRef`] pushed by the [`Writer`], but do something in the meanwhile if we can't.
	///
	/// This is the same as [`Reader::head()`] but if the [`Writer`] is currently
	/// pushing new data, this function will execute the function `F` in the
	/// meanwhile before forcefully acquiring the latest [`CommitRef`] anyway.
	///
	/// This can be any arbitrary code, although the function
	/// is provided with the same [`Reader`], `&self`.
	///
	/// If the [`CommitRef`] could be acquired immediately, then
	/// the function `F` will execute and return.
	///
	/// The parameter `R` is the return value of the function, although
	/// leaving it blank and having a non-returning function will
	/// be enough inference that the return value is `()`.
	///
	/// Basically: "run the function `F` while we're waiting"
	///
	/// ## Example
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// /* Let's just pretend the Writer
	///   is off doing some other things */
	///       std::mem::forget(w);
	///
	/// // Some work to be done.
	/// let mut hello_world   = String::from("hello");
	/// let mut one_two_three = vec![0, 0, 0];
	///
	/// // Pass in a closure, so that we can do
	/// // arbitrary things in the meanwhile...!
	/// let (commit, return_value) = r.head_do(|reader| {
	///     // While we're waiting, let's get some work done.
	///     // Mutate this string.
	///     hello_world.push_str(" world");
	///     // Mutate this vector.
	///     one_two_three[0] = 1;
	///     one_two_three[1] = 2;
	///     one_two_three[2] = 3; // <- `head_do()` returns `()`
	/// });                       // although we could return anything
	///                           // and it would be binded to `return_value`
	///
	/// // We have our commit:
	/// assert_eq!(commit.timestamp(), 0);
	/// // And we did some work
	/// // while waiting to get it:
	/// assert_eq!(hello_world,   "hello world");
	/// assert_eq!(one_two_three, vec![1, 2, 3]);
	/// assert_eq!(return_value,  ());
	/// ```
	pub fn head_do<F, R>(&self, f: F) -> (CommitRef<T>, R)
	where
		F: FnOnce(&Self) -> R
	{
		// Writer is not swapping, acquire head commit.
		if !self.swapping() {
			let head = self.head();
			return (head, f(self));
		}

		// Else execute function and acquire.
		(self.head(), f(self))
	}

	#[inline]
	#[must_use]
	/// Acquire the latest [`CommitRef`] pushed by the [`Writer`] ASAP, but while cooperating
	///
	/// This is the same as [`Reader::head()`] but if the [`Writer`] is currently
	/// pushing new data, this function will spin ([`std::hint::spin_loop`])
	/// until it is not.
	///
	/// Realistically, this function will only spin for a brief moment
	/// as the time it takes [`Writer`] to "push" new data is very small
	/// (1 new `Arc` and an atomic pointer swap).
	pub fn head_spin(&self) -> CommitRef<T> {
		loop {
			if !self.swapping() {
				return self.head();
			}

			std::hint::spin_loop();
		}
	}

	#[inline]
	#[must_use]
	/// Same as [`Self::head_spin`] except use [`std::thread::yield_now`] instead.
	pub fn head_yield(&self) -> CommitRef<T> {
		loop {
			if !self.swapping() {
				return self.head();
			}

			std::thread::yield_now();
		}
	}

	#[inline]
	#[must_use]
	/// Attempt to acquire the latest [`CommitRef`] pushed by the [`Writer`]
	///
	/// This is the same as [`Reader::head()`] but if the [`Writer`] is currently
	/// trying to reclaim old data, this function will return `None`.
	pub fn head_try(&self) -> Option<CommitRef<T>> {
		if self.swapping() {
			None
		} else {
			Some(self.head())
		}
	}

	#[inline]
	#[must_use]
	/// Acquire a [`CommitOwned`] that owns the underlying data
	///
	/// This will expensively clone the underlying data `T`.
	pub fn head_owned(&self) -> CommitOwned<T> {
		self.head().into_commit_owned()
	}

	#[inline]
	/// If the [`Reader`]'s current [`Timestamp`] is greater than an arbitrary [`Commit`]'s [`Timestamp`]
	///
	/// This takes any type of [`Commit`], so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	pub fn ahead_of(&self, commit: &impl Commit<T>) -> bool {
		self.head().ahead(commit)
	}

	#[inline]
	/// If the [`Reader`]'s current [`Timestamp`] is less than an arbitrary [`Commit`]'s [`Timestamp`]
	///
	/// This takes any type of [`Commit`], so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	pub fn behind(&self, commit: &impl Commit<T>) -> bool {
		self.head().behind(commit)
	}

	#[inline]
	#[must_use]
	/// Get the current [`Timestamp`] of the [`Reader`]'s head [`Commit`]
	///
	/// This returns the number indicating the [`Reader`]'s data's version.
	///
	/// This number starts at `0`, increments by `1` every time a [`Writer::commit()`]
	/// -like operation is called, and it will never be greater than the [`Writer`]'s [`Timestamp`].
	pub fn timestamp(&self) -> Timestamp {
		self.head().timestamp()
	}

	#[inline]
	#[must_use]
	#[allow(clippy::missing_panics_doc)]
	/// How many [`Reader`]'s are there?
	///
	/// This is the same as [`Writer::reader_count()`].
	pub fn reader_count(&self) -> NonZeroUsize {
		let count = Arc::strong_count(&self.arc);

		// INVARIANT:
		// The fact that we have are passing an Arc
		// means this will always at-least output 1.
		NonZeroUsize::new(count).expect("reader_count() returned 0")
	}

	#[inline]
	#[must_use]
	/// Is the [`Writer`] currently swapping data?
	///
	/// This indicates if the [`Writer`] very recently [`Writer::push()`]'ed
	/// new data and is about to reclaim old [`Reader`] data.
	///
	/// If this returns `false`, that means subsequently calling [`Reader::head()`]
	/// will return the latest data and not impact the [`Writer`].
	pub fn swapping(&self) -> bool {
		self.swapping.load(Ordering::Acquire)
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
	T: Clone + serde::Serialize
{
	#[inline]
	/// This will call `head()`, then serialize your `T`.
	///
	/// `T::serialize(self.head().data(), serializer)`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (r, _) = someday::new(String::from("hello"));
	///
	/// let json = serde_json::to_string(&r).unwrap();
	/// assert_eq!(json, "\"hello\"");
	/// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        T::serialize(self.head().data(), serializer)
    }
}

#[cfg(feature = "bincode")]
impl<T> bincode::Encode for Reader<T>
where
	T: Clone + bincode::Encode
{
	#[inline]
	/// This will call `head()`, then serialize your `T`.
	///
	/// `T::encode(self.head().data(), encoder)`
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (r, _) = someday::new(String::from("hello"));
	/// let config = bincode::config::standard();
	///
	/// let bytes = bincode::encode_to_vec(&r, config).unwrap();
	/// assert_eq!(bytes, bincode::encode_to_vec(&"hello", config).unwrap());
	/// ```
	fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
		T::encode(self.head().data(), encoder)
	}
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshSerialize for Reader<T>
where
	T: Clone + borsh::BorshSerialize
{
	#[inline]
	/// This will call `self.head().data()`, then serialize your `T`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (r, _) = someday::new(String::from("hello"));
	///
	/// let bytes = borsh::to_vec(&r).unwrap();
	/// assert_eq!(bytes, borsh::to_vec(&"hello").unwrap());
	/// ```
	fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
		T::serialize(self.head().data(), writer)
	}
}