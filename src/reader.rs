//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;
use crate::{
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	Writer,
	Apply,
};

//---------------------------------------------------------------------------------------------------- Reader
/// Reader(s) who can atomically read some data `T`
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
/// 	timestamp: usize,
/// 	data: std::sync::Arc<T>,
/// }
/// ```
/// so as long as that [`CommitRef`] is alive, the data will stay alive.
///
/// These [`CommitRef`]'s are cheaply clonable and sharable with other threads.
///
/// ## Usage
/// This example covers the typical usage of a `Reader`:
/// - Creating some other [`Reader`]'s
/// - Acquring the latest head [`Commit`] of data
/// - Viewing the data, the timestamp
/// - Hanging onto that data for a while
/// - Repeat
///
/// ```rust
/// use someday::{
/// 	{Writer,Reader,Commit,CommitOwned,CommitRef},
/// 	patch::PatchString,
/// };
///
/// // Create a Reader/Writer pair that can "apply"
/// // the `PatchString` patch to `String`'s.
/// let (reader, writer) = someday::new("".into());
///
/// // To clarify the types of these things:
/// // This is the Reader.
/// // It can clone itself infinite amount of
/// // time very cheaply.
/// let reader: Reader<String> = reader;
/// for _ in 0..100 {
/// 	// Pretty cheap operation.
/// 	let another_reader = reader.clone();
/// 	// We can send Reader's to other threads.
/// 	std::thread::spawn(move || assert_eq!(another_reader.head(), ""));
/// }
///
/// // This is the single Writer, it cannot clone itself.
///	let mut writer: Writer<String, PatchString> = writer;
///
/// // Both Reader and Writer are at timestamp 0 and see no changes.
/// assert_eq!(writer.timestamp(), 0);
/// assert_eq!(reader.timestamp(), 0);
/// assert_eq!(*writer.data(), "");
/// assert_eq!(reader.head(), "");
///
/// // Move the Writer into another thread
/// // and make it do some work in the background.
/// std::thread::spawn(move || {
/// 	// 1. Append to string
/// 	// 2. Add the change
/// 	// 3. Commit it
/// 	// 4. Push it so the Readers can see
/// 	// 5. Repeat
/// 	//
/// 	// This is looping at an extremely fast rate
/// 	// and real code probably wouldn't do this, although
/// 	// just for the example...
/// 	loop {
/// 		writer
/// 			.add_and(PatchString::PushStr("abc".into()))
/// 			.commit_and()
/// 			.push();
/// 	}
/// });
///
/// // Wait just a little...
/// std::thread::sleep(std::time::Duration::from_millis(50));
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
/// let mut last_head = reader.head();
/// let mut new_head  = reader.head();
/// for _ in 0..100 {
/// 	last_head = reader.head();
///
/// 	// Wait just a little...
/// 	std::thread::sleep(std::time::Duration::from_millis(50));
///
/// 	new_head = reader.head();
///
/// 	// We got new data!
/// 	assert!(last_head != new_head);
/// 	assert!(last_head.timestamp() < new_head.timestamp());
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
pub struct Reader<T>
where
	T: Clone,
{
	pub(super) arc: Arc<arc_swap::ArcSwapAny<Arc<CommitOwned<T>>>>,
}

impl<T> Reader<T>
where
	T: Clone,
{
	#[inline]
	/// Acquire the latest [`CommitRef`] pushed by the [`Writer`]
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
	/// # use someday::patch::*;
	/// // Create a Reader/Writer pair.
	/// let (r, mut w) = someday::new::<String, PatchString>("".into());
	///
	/// // Both Reader and Writer are at timestamp 0 and see no changes.
	/// assert_eq!(w.timestamp(), 0);
	/// assert_eq!(r.timestamp(), 0);
	/// assert_eq!(w.data(), "");
	/// assert_eq!(r.head(), "");
	///
	/// // Writer commits some changes locally.
	/// w.add_and(PatchString::Set("hello".into())).commit();
	/// // Writer sees local changes.
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(w.data(), "hello");
	///
	/// // Reader does not, because Writer did not `push()`.
	/// let head: CommitRef<String> = r.head();
	/// assert_eq!(head.timestamp(), 0);
	/// assert_eq!(head.data(),      "");
	///
	/// // Writer pushs to the Readers.
	/// w.push();
	///
	/// // Now Readers see changes.
	/// let head: CommitRef<String> = r.head();
	/// assert_eq!(head.timestamp(), 1);
	/// assert_eq!(head.data(),      "hello");
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
		CommitRef {
			inner: arc_swap::Guard::into_inner(self.arc.load()),
		}
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
	/// Get the current [`Timestamp`] of the [`Reader`]'s head [`Commit`]
	///
	/// This returns the number indicating the [`Reader`]'s data's version.
	///
	/// This number starts at `0`, increments by `1` every time a [`Writer::commit()`]
	/// -like operation is called, and it will never be greater than the [`Writer`]'s [`Timestamp`].
	pub fn timestamp(&self) -> Timestamp {
		self.head().timestamp()
	}

	/// Acquire a [`CommitOwned`] that owns the underlying data
	///
	/// This will expensively clone the underlying data `T`.
	pub fn head_owned(&self) -> CommitOwned<T> {
		self.head().into_commit_owned()
	}
}

impl<T: Apply<Patch>, Patch> From<&Writer<T, Patch>> for Reader<T> {
	fn from(value: &Writer<T, Patch>) -> Self {
		value.reader()
	}
}