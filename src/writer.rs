//! Writer<T>

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
};

use crate::{
	INIT_VEC_LEN,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Writer
#[allow(clippy::type_complexity)]
/// The single [`Writer`] of some data `T`
///
/// The [`Writer`]:
/// 1. Stores your `Patch`'s (functions) with [`add()`](Writer::add)
/// 2. Actually applies them to `T` by [`commit()`](Writer::commit)'ing
/// 3. Can [`push()`](Writer::push) so that [`Reader`]'s can see the changes
///
/// The `Writer` can also generate infinite `Reader`'s with [`Writer::reader()`].
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
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head(), "");
///
/// // The Writer can add many `Patch`'s
/// w.add(|w, _| w.push_str("abc"));
/// w.add(|w, _| w.push_str("def"));
/// w.add(|w, _| w.push_str("ghi"));
/// w.add(|w, _| w.push_str("jkl"));
///
/// // But `add()`'ing does not actually modify the
/// // local (Writer) or remote (Readers) data, it
/// // just "stages" them.
/// assert_eq!(w.timestamp(), 0);
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "");
/// assert_eq!(r.head(), "");
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
/// assert_eq!(r.timestamp(), 0);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head(), "");
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
/// assert_eq!(r.timestamp(), 1);
/// assert_eq!(w.data(), "abcdefghi");
/// assert_eq!(r.head(), "abcdefghi");
/// ```
///
/// ## Invariants
/// Some invariants that the `Writer` always upholds, that you can rely on:
/// - [`Writer::timestamp()`] will always be greater than or equal to the [`Reader::timestamp()`]
/// - [`Writer::tags()`] will always return `Commit`'s that were previously [`push()`](Writer::push)'ed
/// - If a `Writer` that is being shared (e.g `Arc<Mutex<Writer<T>>`) panics mid-push, the other `Writer`'s
///   may also panic on any operation that touches local data - i.e. the local data `T` will never
///   be seen in an uninitialized state
/// - `Reader`'s will be completely fine in the case a `Writer` panics mid-push
pub struct Writer<T>
where
	T: Clone,
{
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
	pub(super) local: Option<CommitOwned<T>>,

	/// The current data the remote `Reader`'s can see.
	pub(super) remote: Arc<CommitOwned<T>>,

	/// The AtomicPtr that `Reader`'s enter through.
	/// Calling `.load()` would load the `remote` above.
	pub(super) arc: Arc<arc_swap::ArcSwap<CommitOwned<T>>>,

	/// Patches that have not yet been applied.
	pub(super) patches: Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>>,

	/// Patches that were already applied,
	/// that must be re-applied to the old `T`.
	pub(super) patches_old: Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>>,

	/// This signifies to the `Reader`'s that the
	/// `Writer` is currently attempting to swap data.
	///
	/// `Reader`'s can cooperate by sleeping
	/// for a bit when they see this as `true`
	pub(super) swapping: Arc<AtomicBool>,

	/// Tags.
	pub(super) tags: BTreeMap<Timestamp, CommitRef<T>>,
}

//---------------------------------------------------------------------------------------------------- Writer
impl<T> Writer<T>
where
	T: Clone,
{
	#[inline]
	/// Add a `Patch` to apply to the data `T`
	///
	/// This does not execute the `Patch` immediately,
	/// it will only store it for later usage.
	///
	/// [`Commit`]-like operations are when these
	/// functions are applied to your data, e.g. [`Writer::commit()`].
	///
	/// ```
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<usize>(0);
	///
	/// // Add a patch.
	/// w.add(|w, _| *w += 1);
	///
	/// // It hasn't been applied yet.
	/// assert_eq!(w.staged().len(), 1);
	///
	/// // Now it has.
	/// w.commit();
	/// assert_eq!(w.staged().len(), 0);
	/// ```
	///
	/// # What is a `Patch`?
	/// `Patch` is just a function that will be applied to your data `T`.
	///
	/// The 2 inputs you are given are:
	/// - The [`Writer`]'s local mutable data, `T` (the thing you're modifying)
	/// - The [`Reader`]'s latest head commit
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::sync::*;
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // Use a pre-defined function pointer.
	/// fn fn_ptr(w: &mut String, r: &String) {
	///     w.push_str("hello");
	/// }
	/// w.add(fn_ptr);
	///
	/// // This non-capturing closure gets
	/// // coerced into a `fn(&mut T, &T)`.
	/// w.add(|w, _| {
	///     w.push_str("hello");
	/// });
	///
	/// // This capturing closure turns
	/// // into something that looks like:
	/// // `Box<dyn FnMut(&mut T, &T) + Send + 'static>`
	/// let string: Arc<str> = "hello".into();
	/// w.add(move |w, _| {
	///     let captured = Arc::clone(&string);
	///     w.push_str(&captured);
	/// });
	/// ```
	///
	/// # ⚠️ Non-deterministic `Patch`
	/// The `Patch`'s you use with [`Writer::add`] **must be deterministic**.
	///
	/// The `Writer` may apply your `Patch` twice, so any state that gets
	/// modified or functions used in the `Patch` must result in the
	/// same values as the first time the `Patch` was called.
	///
	/// Here is a **non-deterministic** example:
	/// ```rust
	/// # use someday::*;
	/// # use std::sync::*;
	/// static STATE: Mutex<usize> = Mutex::new(1);
	///
	/// let (_, mut w) = someday::new::<usize>(0);
	///
	/// w.add(move |w, _| {
	///     let mut state = STATE.lock().unwrap();
	///     *state *= 10; // 1*10 the first time, 10*10 the second time...
	///     *w = *state;
	/// });
	/// w.commit();
	/// w.push();
	///
	/// // ⚠️⚠️⚠️ !!!
	/// // The `Writer` reclaimed the old `Reader` data
	/// // and applied our `Patch` again, except, the `Patch`
	/// // was non-deterministic, so now the `Writer`
	/// // and `Reader` have non-matching data...
	/// assert_eq!(*w.data(), 100);
	/// assert_eq!(*w.reader().head(), 10);
	/// ```
	pub fn add<Patch>(&mut self, patch: Patch)
	where
		Patch: FnMut(&mut T, &T) + Send + 'static
	{
		// This used to be:
		//
		// ```rust
		// enum Patch<T> {
		//     Box(Box<FnMut(&mut T, &T) + Send + 'static>),
		//     Fn(fn(&mut T, &T)),
		// }
		// ```
		// so that users could specify non-allocating,
		// non-dynamic-dispatched fn pointers.
		//
		// This was moved onto this function as a generic instead
		// since the type inference and ergonomics was bad.
		//
		// LLVM can optimize out trivial boxes and dyn cases
		// ...but I'm not sure it can when there's multiple
		// mixed `fn`'s and `dyn FnMut()`'s inside a `Vec`.
		//
		// Guess we'll be boxing `fn()`...
		self.patches.push(Box::new(patch));
	}

	#[inline]
	#[allow(clippy::missing_panics_doc)]
	/// Apply all the `Patch`'s that were [`add()`](Writer::add)'ed
	///
	/// The new [`Commit`] created from this will become
	/// the `Writer`'s new [`Writer::head()`].
	///
	/// You can `commit()` multiple times and it will
	/// only affect the `Writer`'s local data.
	///
	/// You can choose when to publish those changes to
	/// the [`Reader`]'s with [`Writer::push()`].
	///
	/// The [`CommitInfo`] object returned is just a container
	/// for some metadata about the `commit()` operation.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<usize>(0);
	///
	/// // Timestamp is 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Add and commit a patch.
	/// w.add(|w, _| *w += 123);
	/// w.commit();
	///
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(*w.head(), 123);
	/// ```
	///
	/// # Timestamp
	/// This will increment the [`Writer`]'s local [`Timestamp`] by `1`,
	/// but only if there were `Patch`'s to actually apply. In other
	/// words, if you did not call [`add()`](Writer::add) before this,
	/// [`commit()`](Writer::commit) will do nothing.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<usize>(0);
	///
	/// // Timestamp is 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // We didn't `add()` anything, but commit anyway.
	/// let commit_info = w.commit();
	/// assert_eq!(commit_info.patches, 0);
	/// assert_eq!(commit_info.timestamp_diff, 0);
	///
	/// // There was nothing to commit,
	/// // so our timestamp did not change.
	/// assert_eq!(w.timestamp(), 0);
	/// assert_eq!(*w.head(), 0);
	/// ```
	pub fn commit(&mut self) -> CommitInfo {
		let patch_len = self.patches.len();

		// Early return if there was nothing to do.
		if patch_len == 0 {
			return CommitInfo {
				patches: 0,
				timestamp_diff: self.timestamp_diff(),
			};
		}

		self.local_as_mut().timestamp += 1;

		// Pre-allocate some space for the new patches.
		self.patches_old.reserve_exact(patch_len);

		// Apply the patches and add to the old vector.
		for mut patch in self.patches.drain(..) {
			patch(
				// We can't use `self.local_as_mut()` here
				// We can't have `&mut self` and `&self`.
				//
				// INVARIANT: local must be initialized after push()
				&mut self.local.as_mut().unwrap().data,
				&self.remote.data,
			);
			self.patches_old.push(patch);
		}

		CommitInfo {
			patches: patch_len,
			timestamp_diff: self.timestamp_diff(),
		}
	}

	#[inline]
	/// Conditionally push [`Writer`]'s local _committed_ data to the [`Reader`]'s.
	///
	/// This will only push changes if there are new [`Commit`]'s
	/// (i.e if [`Writer::synced`] returns `true`).
	///
	/// This may be expensive as there are other operations in this
	/// function (memory reclaiming, re-applying patches).
	///
	/// This will return how many `Commit`'s the `Writer`'s pushed.
	///
	/// `Reader`'s will atomically be able to access the
	/// the new `Commit` before this function is over.
	///
	/// The `Patch`'s that were not [`commit()`](Writer::commit)'ed will not be
	/// pushed and will remain in the [`staged()`](Writer::staged) vector of patches.
	///
	/// The [`PushInfo`] object returned is just a container
	/// for some metadata about the [`push()`](Writer::push) operation.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	/// w.add(|w, _| w.push_str("abc"));
	///
	/// // This call does nothing since
	/// // we haven't committed anything.
	/// let push_info = w.push();
	/// assert_eq!(push_info.timestamp, 0);
	/// assert_eq!(push_info.commits, 0);
	/// assert_eq!(push_info.reclaimed, false);
	///
	/// // Now there are commits to push.
	/// w.commit();
	///
	/// if w.ahead() {
	///     let push_info = w.push();
	///     // We pushed 1 commit.
	///     assert_eq!(push_info.timestamp, 1);
	///     assert_eq!(push_info.commits, 1);
	///     assert_eq!(push_info.reclaimed, true);
	/// } else {
	///     // this branch cannot happen
	///     unreachable!();
	/// }
	/// ```
	pub fn push(&mut self) -> PushInfo {
		self.push_inner::<false, ()>(None, None::<fn()>).0
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it will [`std::thread::sleep()`] for at least `duration`
	/// amount of time to wait to reclaim the old [`Reader`]'s data.
	///
	/// If `duration` has passed, the [`Writer`] will
	/// clone the data as normal and continue on.
	///
	/// This is useful if you know your `Reader`'s only
	/// hold onto old data for a brief moment.
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{sync::*,thread::*,time::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// # let barrier  = Arc::new(Barrier::new(2));
	/// # let other_b = barrier.clone();
	/// let commit = r.head();
	/// spawn(move || {
	///     # other_b.wait();
	///     // This `Reader` is holding onto the old data.
	///     let moved = commit;
	///     // But will let go after 1 millisecond.
	///     sleep(Duration::from_millis(1));
	/// });
	///
	/// # barrier.wait();
	/// // Wait 250 milliseconds before resorting to cloning data.
	/// let commit_info = w.push_wait(Duration::from_millis(250));
	/// // We pushed 1 commit.
	/// assert_eq!(commit_info.commits, 1);
	/// // And we successfully reclaimed the old data cheaply.
	/// assert_eq!(commit_info.reclaimed, true);
	/// ```
	pub fn push_wait(&mut self, duration: Duration) -> PushInfo {
		self.push_inner::<false, ()>(Some(duration), None::<fn()>).0
	}

	#[inline]
	#[allow(clippy::missing_panics_doc)]
	/// This function is the same as [`Writer::push()`]
	/// but it will execute the function `F` in the meanwhile before
	/// attempting to reclaim the old [`Reader`] data.
	///
	/// The generic `R` is the return value of the function.
	/// Leaving it blank and having a non-returning function will
	/// be enough inference that the return value is `()`.
	///
	/// Basically: "run the function `F` while we're waiting"
	///
	/// This is useful to get some work done before waiting
	/// on the `Reader`'s to drop old copies of data.
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{sync::*,thread::*,time::*,collections::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// # let barrier  = Arc::new(Barrier::new(2));
	/// # let other_b = barrier.clone();
	/// let head = r.head();
	/// spawn(move || {
	///     # other_b.wait();
	///     // This `Reader` is holding onto the old data.
	///     let moved = head;
	///     // But will let go after 100 milliseconds.
	///     sleep(Duration::from_millis(100));
	/// });
	///
	/// # barrier.wait();
	/// // Some work to be done.
	/// let mut hashmap = HashMap::<usize, String>::new();
	/// let mut vec     = vec![];
	///
	/// // Commit.
	/// // Now the `Writer` is ahead by 1 commit, while
	/// // the `Reader` is hanging onto the old one.
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// // Pass in a closure, so that we can do
	/// // arbitrary things in the meanwhile...!
	/// let (push_info, return_value) = w.push_do(|| {
	///     // While we're waiting, let's get some work done.
	///     // Add a bunch of data to this HashMap.
	///     (0..1_000).for_each(|i| {
	///         hashmap.insert(i, format!("{i}"));
	///     });
	///     // Add some data to the vector.
	///     (0..1_000).for_each(|_| {
	///         vec.push(format!("aaaaaaaaaaaaaaaa"));
	///     }); // <- `push_do()` returns `()`
	///     # sleep(Duration::from_secs(1));
	/// });     // although we could return anything
	///         // and it would be binded to `return_value`
	///
	/// // At this point, the old `Reader`'s have
	/// // probably all dropped their old references
	/// // and we can probably cheaply reclaim our
	/// // old data back.
	///
	/// // And yes, looks like we got it back cheaply:
	/// assert_eq!(push_info.reclaimed, true);
	///
	/// // And we did some work
	/// // while waiting to get it:
	/// assert_eq!(hashmap.len(), 1_000);
	/// assert_eq!(vec.len(), 1_000);
	/// assert_eq!(return_value, ());
	/// ```
	pub fn push_do<F, R>(&mut self, f: F) -> (PushInfo, R)
	where
		F: FnOnce() -> R
	{
		let (push_info, r) = self.push_inner::<false, R>(None, Some(f));

		// INVARIANT: we _know_ `R` will be a `Some`
		// because we provided a `Some`. `push_inner()`
		// will always return a Some(value).
		(push_info, r.unwrap())
	}

	#[inline]
	/// This function is the same as [`Writer::push()`]
	/// but it will **always** clone the data
	/// and not attempt to reclaim any old data.
	///
	/// This is useful if you know reclaiming old data
	/// and re-applying your commits would take longer and/or
	/// be more expensive than cloning the data itself.
	///
	/// Or if you know your `Reader`'s will be holding
	/// onto the data for a long time, and reclaiming data
	/// will be unlikely.
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// let commit = r.head();
	/// spawn(move || {
	///     // This `Reader` will hold onto the old data forever.
	///     let moved = commit;
	///     loop { std::thread::park(); }
	/// });
	///
	/// // Always clone data, don't wait.
	/// let push_status = w.push_clone();
	/// // We pushed 1 commit.
	/// assert_eq!(push_status.commits, 1);
	/// ```
	pub fn push_clone(&mut self) -> PushInfo {
		// If we're always cloning, there's no
		// need to block Reader's for reclamation
		// so don't set `swapping`.
		self.push_inner::<true, ()>(None, None::<fn()>).0
	}

	/// Generic function to handle all the different types of pushes.
	fn push_inner<const CLONE: bool, R>(
		&mut self,
		duration: Option<Duration>,
		function: Option<impl FnOnce() -> R>,
	) -> (PushInfo, Option<R>)
	{
		// Early return if no commits.
		if self.synced() {
			let return_value = function.map(|f| f());
			return (PushInfo {
				timestamp: self.timestamp(),
				commits: 0,
				reclaimed: false,
			}, return_value);
		}

		// Set atomic bool to indicate to `Reader`'s
		// that we're about to start reclaiming.
		if !CLONE { self.swapping.store(true, Ordering::Release); }

		// INVARIANT: we're temporarily "taking" our `self.local`.
		// It will be uninitialized for the time being.
		// We need to initialize it before returning.
		let local = self.local.take().unwrap();
		// Swap the reader's `arc_swap` with our new local.
		let old = self.arc.swap(Arc::new(local));

		if !CLONE { self.swapping.store(false, Ordering::Release); }

		// To keep the "swapping" phase as small
		// as possible to not block `Reader`'s, these
		// operations are done here.
		//
		// `self.arc` now returns the new data.
		self.remote = self.arc.load_full();
		let timestamp_diff = self.remote.timestamp - old.timestamp;

		// Return early if the user wants to deep-clone no matter what.
		if CLONE {
			self.local = Some((*self.remote).clone());
			self.patches_old.clear();
			return (PushInfo {
				timestamp: self.remote.timestamp,
				commits: timestamp_diff,
				reclaimed: false,
			}, None)
		}

		// If the user wants to execute a function
		// while waiting, do so and get the return value.
		let return_value = function.map(|f| f());

		// Try to reclaim data.
		let (mut local, reclaimed) = match Arc::try_unwrap(old) {
			// If there are no more dangling readers on the
			// old Arc we can cheaply reclaim the old data.
			Ok(old) => (old, true),

			// Else, if the user wants to
			// sleep and try again, do so.
			Err(old) => {
				if let Some(duration) = duration {
					// Sleep.
					std::thread::sleep(duration);
					// Try again.
					if let Some(old) = Arc::into_inner(old) {
						(old, true)
					} else {
						((*self.remote).clone(), false)
					}
				} else {
					// Else, there are dangling readers left.
					// As to not wait on them, just expensively clone
					// the inner data to have a mutually exclusive
					// up-to-date local copy.
					((*self.remote).clone(), false)
				}
			},
		};

		// INVARIANT:
		// `self.swapping` must be `false` before we
		// return or else we will lock `Reader`'s
		debug_assert!(
			!self.swapping.load(Ordering::SeqCst),
			"Writer's `swapping` is still true even after swap"
		);

		if reclaimed {
			// Re-apply patches to this old data.
			for mut patch in self.patches_old.drain(..) {
				patch(&mut local.data, &self.remote.data);
			}
			// Set proper timestamp if we're reusing old data.
			local.timestamp = self.remote.timestamp;
		}

		// Re-initialize `self.local`.
		self.local = Some(local);

		// Clear functions.
		self.patches_old.clear();

		// Output how many commits we pushed.
		(PushInfo {
			timestamp: self.remote.timestamp,
			commits: timestamp_diff,
			reclaimed
		}, return_value)
	}

	#[inline]
	#[allow(clippy::missing_panics_doc)]
	/// [`add()`](Writer::add), [`commit()`](Writer::commit), and [`push()`](Writer::push)
	///
	/// This function combines `add()`, `commit()`, `push()` together.
	/// Since these actions are done together, return values are allowed
	/// to be specified where they wouldn't otherwise be.
	///
	/// The input `Patch` no longer has to be `Send + 'static` either.
	///
	/// This allows you to specify any arbitrary return value from
	/// your `Patch`'s, even return values from your `T` itself.
	///
	/// For example, if you'd like to receive a large chunk of data
	/// from your `T` instead of throwing it away:
	/// ```rust
	/// # use someday::*;
	/// // Very expensive data.
	/// let vec = (0..100_000).map(|i| format!("{i}")).collect();
	///
	/// let (_, mut w) = someday::new::<Vec<String>>(vec);
	/// assert_eq!(w.timestamp(), 0);
	/// assert_eq!(w.timestamp_remote(), 0);
	///
	/// let (info, r1, r2) = w.add_commit_push(|w, _| {
	///     // Swap our value, and get back the strings.
	///     // This implicitly becomes our <Return> (Vec<String>).
	///     std::mem::take(w)
	/// });
	///
	/// // We got our 100,000 `String`'s back
	/// // instead of dropping them!
	/// let r1: Vec<String> = r1;
	/// // If `Writer` reclaimed data and applied our
	/// // `Patch` to it, it also got returned!
	/// let r2: Option<Vec<String>> = r2;
	/// // And, some push info.
	/// let info: PushInfo = info;
	///
	/// // We got back our original strings.
	/// for (i, string) in r1.into_iter().enumerate() {
	///     assert_eq!(format!("{i}"), string);
	/// }
	///
	/// // If the `Writer` reclaimed data,
	/// // then `r2` will _always_ be a `Some`.
	/// if info.reclaimed {
	///     // This also contains 100,000 strings.
	///     assert!(r2.is_some());
	/// }
	///
	/// // And the `Patch` got applied to the `Writer`'s data.
	/// assert!(w.data().is_empty());
	/// assert_eq!(w.timestamp(), 1);
	/// assert_eq!(w.timestamp_remote(), 1);
	/// ```
	///
	/// # Generics
	/// The generic inputs are:
	/// - `Patch`
	/// - `Return`
	///
	/// `Patch` is the same as [`Writer::add()`] however, it has a
	/// `-> Return` value associated with it, this is defined by
	/// you, using the `Return` generic.
	///
	/// # Returned Tuple
	/// The returned tuple is contains the regular [`PushInfo`]
	/// along with a `Return` and `Option<Return>`.
	///
	/// The `Return` is the data returned by operating on the `Writer`'s side
	/// of the data.
	///
	/// The `Option<Return>` is `Some` if the `Writer` reclaimed the `Reader`'s
	/// side of the data, and re-applied your `Patch` - it returns it instead
	/// of dropping it. This means that if `PushInfo`'s `reclaimed` is
	/// `true`, this `Option<Return>` will _always_ be `Some`.
	///
	/// # Timestamp
	/// This function will always increment the [`Writer`]'s local [`Timestamp`] by `1`.
	pub fn add_commit_push<Patch, Return>(&mut self, mut patch: Patch) -> (PushInfo, Return, Option<Return>)
	where
		// We're never storing this `Patch` so it
		// doesn't have to be `Send + 'static`.
		Patch: FnMut(&mut T, &T) -> Return
	{
		// Commit `Patch` to our local data.
		self.local_as_mut().timestamp += 1;
		let return_1 = patch(
			&mut self.local.as_mut().unwrap().data,
			&self.remote.data,
		);

		// Push all commits so far.
		let push_info = self.push();

		// If the `Writer` reclaimed data, we must re-apply
		// since we did not push the Patch onto the `patches_old` Vec
		// (since we want the return value).
		let return_2 = push_info.reclaimed.then(|| {
			patch(
				&mut self.local.as_mut().unwrap().data,
				&self.remote.data,
			)
		});

		(push_info, return_1, return_2)
	}

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
			swapping: Arc::clone(&self.swapping)
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
	/// assert_eq!(r.head(),  0);
	///
	/// // Writer commits some changes.
	/// w.add(|w, _| *w += 1);
	/// w.commit();
	///
	/// //  Writer sees local change.
	/// assert_eq!(*w.data(), 1);
	/// // Reader doesn't see change.
	/// assert_eq!(r.head(), 0);
	/// ```
	pub fn data(&self) -> &T {
		self.local_as_ref()
	}

	#[inline]
	/// View the latest copy of data [`Reader`]'s have access to
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut w) = someday::new::<usize>(0);
	///
	/// // Writer commits some changes.
	/// w.add(|w, _| *w += 1);
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
	/// let commit: &CommitOwned<usize> = w.head();
	/// assert_eq!(commit.timestamp, 0);
	/// assert_eq!(commit.data, 500);
	///
	/// // Writer commits some changes.
	/// w.add(|w, _| *w += 1);
	/// w.commit();
	///
	/// // Head commit is now changed.
	/// let commit: &CommitOwned<usize> = w.head();
	/// assert_eq!(commit.timestamp, 1);
	/// assert_eq!(commit.data, 501);
	/// ```
	pub const fn head(&self) -> &CommitOwned<T> {
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
	/// let commit: &CommitOwned<usize> = w.head_remote();
	/// assert_eq!(commit.timestamp(), 0);
	/// assert_eq!(*commit.data(), 500);
	///
	/// // Writer commits & pushes some changes.
	/// w.add(|w, _| *w += 1);
	/// w.commit();
	/// w.push();
	///
	/// // Reader's head commit is now changed.
	/// let commit: &CommitOwned<usize> = w.head_remote();
	/// assert_eq!(commit.timestamp(), 1);
	/// assert_eq!(*commit.data(), 501);
	/// ```
	pub fn head_remote(&self) -> &CommitOwned<T> {
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
		CommitRef { inner: Arc::clone(&self.remote) }
	}

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
	/// w.add(|w, _| w.push_str("hello"));
	/// w.commit();
	/// assert_eq!(w.head(), "hello");
	///
	/// // Reader's sees nothing
	/// assert_eq!(r.head(), "");
	///
	/// // Pull from the Reader.
	/// let pull_status: PullInfo<String> = w.pull().unwrap();
	/// assert_eq!(pull_status.old_writer_data, "hello");
	///
	/// // We're back to square 1.
	/// assert_eq!(w.head(), "");
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
		let old_writer_data = self.local.take().unwrap();
		self.local = Some((*self.remote).clone());

		// Delete old functions, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.patches_old.clear();

		Some(PullInfo {
			commits_reverted,
			old_writer_data,
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
	/// A `Patch` that overwrites the data
	/// applied with `commit()` would be
	/// equivalent to this convenience function.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Push changes.
	/// w.add(|w, _| w.push_str("hello"));
	/// w.commit(); // <- commit 1
	/// w.push();
	///
	/// assert_eq!(w.timestamp(), 1);
	///
	/// // Reader's sees them.
	/// assert_eq!(r.head(), "hello");
	/// assert_eq!(r.timestamp(), 1);
	///
	/// // Commit some changes.
	/// w.add(|w, _| *w = "hello".into());
	/// w.commit(); // <- commit 2
	/// w.add(|w, _| *w = "hello".into());
	/// w.commit(); // <- commit 3
	/// w.add(|w, _| *w = "hello".into());
	/// w.commit(); // <- commit 4
	/// assert_eq!(w.committed_patches().len(), 3);
	///
	/// // Overwrite the Writer with arbitrary data.
	/// let old_data = w.overwrite(String::from("world")); // <- commit 5
	/// assert_eq!(old_data, "hello");
	/// // Committed functions were deleted.
	/// assert_eq!(w.committed_patches().len(), 0);
	///
	/// // Push that change.
	/// w.push();
	///
	/// // Readers see change.
	/// assert_eq!(r.head(), "world");
	///
	/// // 5 commits total.
	/// assert_eq!(w.timestamp(), 5);
	/// assert_eq!(r.timestamp(), 5);
	/// ```
	///
	/// ## Timestamp
	/// This increments the `Writer`'s local `Timestamp` by `1`.
	pub fn overwrite(&mut self, data: T) -> CommitOwned<T> {
		// Delete old functions, we won't need
		// them anymore since we just overwrote
		// our data anyway.
		self.patches_old.clear();

		// INVARIANT: `local` must be initialized after push()
		let timestamp = self.timestamp() + 1;
		let old_data = self.local.take().unwrap();

		self.local = Some(CommitOwned {
			timestamp,
			data
		});

		old_data
	}

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
	/// w.add(|w, _| w.push_str("a"));
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
	///     w.add(|w, _| w.push_str("b"));
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
	/// let inner_data: String = tag.try_unwrap().unwrap().data;
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
			.or_insert_with(|| CommitRef { inner: Arc::clone(&self.remote) })
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
	/// w.add(|w, _| w.push_str("a"));
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
	///     w.add(|w, _| w.push_str("a"));
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
	///     writer.add(|w, _| *w = "bbb".into());
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
	pub fn tag_retain<F>(&mut self, f: F)
	where
		F: Fn(&CommitRef<T>) -> bool,
	{
		// The normal `retain()` gives `&mut` access to the
		// 2nd argument, but we need to uphold the invariant
		// that timestamps + commits are always valid, and
		// never randomly mutated by the user.
		//
		// So, we will iterate over our btree, marking
		// which timestamps we need to remove, then sweep
		// them all after.
		self.tags
			.iter_mut()                  // for each key, value
			.filter(|(_, v)| !f(v))      // yield if F returns false
			.map(|(k, _)| *k)            // yield the timestamp
			.collect::<Vec<Timestamp>>() // collect them
			.into_iter()                 // for each timestamp
			.for_each(|timestamp| { self.tags.remove(&timestamp); }); // remove it
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
	/// w.add(move |w, _| {
	///     let mut state = STATE.lock().unwrap();
	///     *state *= 10; // 1*10 the first time, 10*10 the second time...
	///     *w = *state;
	/// });
	/// w.commit();
	/// w.push();
	///
	/// // Same timestamps...
	/// assert_eq!(w.timestamp(), w.reader().head().timestamp());
	///
	/// // ⚠️ Out of sync data!
	/// assert_eq!(*w.data(), 100);
	/// assert_eq!(*w.reader().head(), 10);
	///
	/// // But, this function tells us the truth.
	/// assert_eq!(w.diff(), true);
	/// ```
	pub fn diff(&self) -> bool
	where T:
		PartialEq<T>
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
	/// [`Writer` is ahead of the [`Reader`]'s)
	///
	/// Note that this does not check the data itself, only the `Timestamp`.
	///
	/// ```rust
	/// # use someday::*;
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Commit 10 times but don't push.
	/// for i in 0..10 {
	///     w.add(|w, _| w.push_str("abc"));
	///     w.commit();
	/// }
	///
	/// // Writer at timestamp 10.
	/// assert_eq!(w.timestamp(), 10);
	///
	/// // Reader at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
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
	/// This takes any type of `Commit`, so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // Commit 10 times.
	/// for i in 0..10 {
	///     w.add(|w, _| w.push_str("abc"));
	///     w.commit();
	/// }
	/// // At timestamp 10.
	/// assert_eq!(w.timestamp(), 10);
	///
	/// // Create fake `CommitOwned`
	/// let fake_commit = CommitOwned {
	///     timestamp: 1,
	///     data: String::new(),
	/// };
	///
	/// // Writer is ahead of that commit.
	/// assert!(w.ahead_of(&fake_commit));
	/// ```
	pub fn ahead_of(&self, commit: &impl Commit<T>) -> bool {
		self.local_as_ref().ahead(commit)
	}

	#[inline]
	#[allow(clippy::missing_panics_doc)]
	/// If the [`Writer`]'s local [`Timestamp`] is less than an arbitrary [`Commit`]'s `Timestamp`
	///
	/// This takes any type of `Commit`, so either [`CommitRef`] or [`CommitOwned`] can be used as input.
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // At timestamp 0.
	/// assert_eq!(w.timestamp(), 0);
	///
	/// // Create fake `CommitOwned`
	/// let fake_commit = CommitOwned {
	///     timestamp: 1000,
	///     data: String::new(),
	/// };
	///
	/// // Writer is behind that commit.
	/// assert!(w.behind(&fake_commit));
	/// ```
	pub fn behind(&self, commit: &impl Commit<T>) -> bool {
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
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// // At timestamp 1.
	/// assert_eq!(w.timestamp(), 1);
	/// // We haven't pushed, so Reader's
	/// // are still at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
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
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// // Writer is at timestamp 1.
	/// assert_eq!(w.timestamp(), 1);
	/// // We haven't pushed, so Reader's
	/// // are still at timestamp 0.
	/// assert_eq!(r.timestamp(), 0);
	///
	/// // Push changes
	/// w.push();
	///
	/// // Readers are now up-to-date.
	/// assert_eq!(r.timestamp(), 1);
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
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	/// w.push();
	///
	/// // Commit 5 changes locally.
	/// for i in 0..5 {
	///     w.add(|w, _| w.push_str("abc"));
	///     w.commit();
	/// }
	///
	/// // Writer is at timestamp 5.
	/// assert_eq!(w.timestamp(), 6);
	/// // Reader's are still at timestamp 1.
	/// assert_eq!(r.timestamp(), 1);
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
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	/// w.push();
	///
	/// // Commit 5 changes locally.
	/// for i in 0..5 {
	///     w.add(|w, _| w.push_str("abc"));
	///     w.commit();
	/// }
	///
	/// // Writer is at timestamp 5.
	/// assert_eq!(w.timestamp(), 6);
	/// // Reader's are still at timestamp 1.
	/// assert_eq!(r.timestamp(), 1);
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
	/// w.add(|w, _| w.push_str("abc"));
	/// assert_eq!(w.staged().len(), 1);
	///
	/// // Restore changes.
	/// let drain = w.restore();
	/// assert_eq!(drain.count(), 1);
	/// ```
	pub fn restore(&mut self) -> std::vec::Drain<'_, Box<dyn FnMut(&mut T, &T) + Send + 'static>> {
		self.patches.drain(..)
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
	/// w.add(|w, _| w.push_str("abc"));
	///
	/// // We see and mutate the staged changes.
	/// assert_eq!(w.staged().len(), 1);
	///
	/// // Let's actually remove that change.
	/// let removed = w.staged().remove(0);
	/// assert_eq!(w.staged().len(), 0);
	/// ```
	pub fn staged(&mut self) -> &mut Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>> {
		&mut self.patches
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
	pub const fn tags(&self) -> &BTreeMap<Timestamp, CommitRef<T>> {
		&self.tags
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
	/// w.add(|w, _| w.push_str("abc"));
	/// w.commit();
	///
	/// // We can see but not mutate functions.
	/// assert_eq!(w.committed_patches().len(), 1);
	/// ```
	pub fn committed_patches(&self) -> &Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>> {
		&self.patches_old
	}

	#[inline]
	/// How many [`Reader`]'s are _currently_ accessing
	/// the current `Reader` head [`Commit`]?
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (_, mut w) = someday::new::<String>("".into());
	///
	/// // The Writer, `w` holds 2 strong counts.
	/// assert_eq!(w.head_readers(), 2);
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
	/// assert_eq!(w.head_readers(), 2);
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
	/// assert_eq!(w.head_readers(), 10);
	/// ```
	pub fn head_readers(&self) -> usize {
		Arc::strong_count(&self.remote)
	}

	#[inline]
	/// How many [`Reader`]'s are there?
	///
	/// Unlike [`Writer::head_readers()`], this doesn't count references
	/// to the current data, it counts how many `Reader` objects are in existence.
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // 2 Reader's (the Writer counts as a Reader).
	/// assert_eq!(w.reader_count(), 2);
	///
	/// // Create and leak 8 Reader's.
	/// for i in 0..8 {
	///     let reader = r.clone();
	///     std::mem::forget(reader);
	/// }
	///
	/// // Now there are 10.
	/// assert_eq!(w.reader_count(), 10);
	/// ```
	pub fn reader_count(&self) -> usize {
		Arc::strong_count(&self.arc)
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
			head_readers: self.head_readers(),
			reader_count: self.reader_count(),
			timestamp: self.timestamp(),
			timestamp_remote: self.timestamp_remote(),
		}
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
	/// let (_, mut w) = someday::new_with_capacity::<String>("".into(), 16);
	///
	/// // Capacity is 16.
	/// assert_eq!(w.committed_patches().capacity(), 16);
	/// assert_eq!(w.staged().capacity(),            16);
	///
	/// // Commit 32 `Patch`'s
	/// for i in 0..32 {
	///     w.add(|w, _| *w = "".into());
	///     w.commit();
	/// }
	/// // Stage 16 `Patch`'s
	/// for i in 0..16 {
	///     w.add(|w, _| *w = "".into());
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

	#[allow(clippy::missing_panics_doc, clippy::type_complexity)]
	/// Consume this [`Writer`] and return the inner components
	///
	/// In left-to-right order, this returns:
	/// 1. The `Writer`'s local data
	/// 2. The latest [`Reader`]'s [`Commit`] (aka, from [`Reader::head()`])
	/// 3. The "staged" `Patch`'s that haven't been [`commit()`](Writer::commit)'ed (aka, from [`Writer::staged()`])
	/// 4. The committed `Patch`'s that haven't been [`push()`](Writer::push)'ed (aka, from [`Writer::committed_patches()`])
	/// 5. [`Writer::tags()`]
	///
	/// ```rust
	/// # use someday::*;
	/// # use std::{thread::*,time::*};
	/// let (r, mut w) = someday::new::<String>("".into());
	///
	/// // Commit some changes.
	/// w.add(|w, _| w.push_str("a"));
	/// w.commit();
	/// w.tag();
	///
	/// // Add but don't commit.
	/// w.add(|w, _| w.push_str("b"));
	///
	/// let (
	///     writer_data,
	///     reader_data,
	///     staged_changes,
	///     committed_changes,
	///     tags,
	/// ) = w.into_inner();
	///
	/// assert_eq!(writer_data, "a");
	/// assert_eq!(reader_data, ""); // We never `push()`'ed, so Readers saw nothing.
	/// assert_eq!(staged_changes.len(), 1);
	/// assert_eq!(committed_changes.len(), 1);
	/// assert_eq!(tags.len(), 1);
	/// ```
	pub fn into_inner(self) -> (
		CommitOwned<T>,
		CommitRef<T>,
		Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>>,
		Vec<Box<dyn FnMut(&mut T, &T) + Send + 'static>>,
		BTreeMap<Timestamp, CommitRef<T>>,
	) {
		(
			// INVARIANT: local must be initialized after push()
			self.local.unwrap(),
			CommitRef { inner: self.remote },
			self.patches,
			self.patches_old,
			self.tags,
		)
	}
}

//---------------------------------------------------------------------------------------------------- Private writer functions
impl<T> Writer<T>
where
	T: Clone
{
	#[allow(clippy::option_if_let_else,clippy::inline_always)]
	#[inline(always)]
	/// Borrow `self.local`.
	const fn local_as_ref(&self) -> &CommitOwned<T> {
		// INVARIANT: `local` must be initialized after push()
		match self.local.as_ref() {
			Some(local) => local,
			_ => panic!("writer.local was not initialized after push()"),
		}
	}

	#[allow(clippy::option_if_let_else,clippy::inline_always)]
	#[inline(always)]
	/// Borrow `self.local`.
	fn local_as_mut(&mut self) -> &mut CommitOwned<T> {
		// INVARIANT: `local` must be initialized after push()
		match self.local.as_mut() {
			Some(local) => local,
			_ => panic!("writer.local was not initialized after push()"),
		}
	}

	#[allow(clippy::option_if_let_else,clippy::inline_always)]
	#[inline(always)]
	/// Same as `local_as_mut()`, but field specific so we
	/// can around taking `&mut self` when we need
	/// `&` to `self` as well.
	///
	/// INVARIANT: This function is ONLY for this `self.local` purpose.
	fn local_field(local: &mut Option<CommitOwned<T>>) -> &mut CommitOwned<T> {
		// INVARIANT: `local` must be initialized after push()
		match local {
			Some(local) => local,
			_ => panic!("writer.local was not initialized after push()"),
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
			.field("swapping", &self.swapping)
			.field("tags", &self.tags)
			.finish_non_exhaustive()
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
	/// let (_, w1) = someday::default::<usize>();
	/// let w2      = Writer::<usize>::default();
	///
	/// assert_eq!(*w1.data(), 0);
	/// assert_eq!(*w2.data(), 0);
	/// ```
	fn default() -> Self {
		crate::default().1
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

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Writer<T>
where
	T: Clone + serde::Serialize
{
	#[inline]
	/// This will call `data()`, then serialize your `T`.
	///
	/// `T::serialize(self.data(), serializer)`
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let json = serde_json::to_string(&w).unwrap();
	/// assert_eq!(json, "\"hello\"");
	/// ```
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
		T::serialize(self.data(), serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Writer<T>
where
	T: Clone + serde::Deserialize<'de>
{
	#[inline]
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// `T::deserialize(deserializer).map(|t| crate::new(t).1)`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let json = serde_json::to_string(&w).unwrap();
	/// assert_eq!(json, "\"hello\"");
	///
	/// let writer: Writer<String> = serde_json::from_str(&json).unwrap();
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>
	{
		T::deserialize(deserializer).map(|t| crate::new(t).1)
	}
}

#[cfg(feature = "bincode")]
impl<T> bincode::Encode for Writer<T>
where
	T: Clone + bincode::Encode
{
	#[inline]
	/// This will call `data()`, then serialize your `T`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	/// let config = bincode::config::standard();
	///
	/// let bytes = bincode::encode_to_vec(&w, config).unwrap();
	/// assert_eq!(bytes, bincode::encode_to_vec(&"hello", config).unwrap());
	/// ```
	fn encode<E: bincode::enc::Encoder>(&self, encoder: &mut E) -> Result<(), bincode::error::EncodeError> {
		T::encode(self.data(), encoder)
	}
}

#[cfg(feature = "bincode")]
impl<T> bincode::Decode for Writer<T>
where
	T: Clone + bincode::Decode
{
	#[inline]
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	/// let config = bincode::config::standard();
	///
	/// let bytes = bincode::encode_to_vec(&w, config).unwrap();
	/// assert_eq!(bytes, bincode::encode_to_vec(&"hello", config).unwrap());
	///
	/// let writer: Writer<String> = bincode::decode_from_slice(&bytes, config).unwrap().0;
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn decode<D: bincode::de::Decoder>(decoder: &mut D) -> Result<Self, bincode::error::DecodeError> {
		T::decode(decoder).map(|t| crate::new(t).1)
	}
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshSerialize for Writer<T>
where
	T: Clone + borsh::BorshSerialize
{
	/// This will call `data()`, then serialize your `T`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let bytes = borsh::to_vec(&w).unwrap();
	/// assert_eq!(bytes, borsh::to_vec(&"hello").unwrap());
	/// ```
	fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
		T::serialize(self.data(), writer)
	}
}

#[cfg(feature = "borsh")]
impl<T> borsh::BorshDeserialize for Writer<T>
where
	T: Clone + borsh::BorshDeserialize
{
	/// This will deserialize your data `T` directly into a `Writer`.
	///
	/// ```rust
	/// # use someday::*;
	///
	/// let (_, w) = someday::new(String::from("hello"));
	///
	/// let bytes = borsh::to_vec(&w).unwrap();
	/// assert_eq!(bytes, borsh::to_vec(&"hello").unwrap());
	///
	/// let writer: Writer<String> = borsh::from_slice(&bytes).unwrap();
	/// assert_eq!(writer.data(), "hello");
	/// ```
	fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
		T::deserialize_reader(reader).map(|t| crate::new(t).1)
	}
}