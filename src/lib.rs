#![doc = include_str!("../README.md")]

//---------------------------------------------------------------------------------------------------- Docs
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

//---------------------------------------------------------------------------------------------------- Lints
#![forbid(
	future_incompatible,
	let_underscore,
	break_with_label_and_loop,
	coherence_leak_check,
	deprecated,
	duplicate_macro_attributes,
	exported_private_dependencies,
	for_loops_over_fallibles,
	large_assignments,
	overlapping_range_endpoints,
	private_in_public,
	semicolon_in_expressions_from_macros,
	redundant_semicolons,
	unconditional_recursion,
	unused_allocation,
	unused_braces,
	unused_doc_comments,
	unused_labels,
	unused_unsafe,
	while_true,
	keyword_idents,
	missing_docs,
	non_ascii_idents,
	noop_method_call,
	unreachable_pub,
	single_use_lifetimes,
	variant_size_differences,
	unused_mut,
)]
#![deny(
	unused_comparisons,
	nonstandard_style,
)]

//---------------------------------------------------------------------------------------------------- Mod
mod reader;
mod commit;
mod writer;
mod apply;

/// Metadata resulting from common [`Writer`] operations
///
/// These are simple container structs that hold
/// information about [`Writer`] operations.
pub mod info;
pub use info::*;

#[cfg(feature = "patch")]
/// Objects implementing [`Apply`] on common data structures
///
/// These are very basic common operations that can be done to common objects.
///
/// Anything with trait bounds or complicated returned values is not enumerated.
///
/// These all implement [`Apply`], so that they can
/// be used as "patches" to give to your [`Writer`].
///
/// Realistically, you should be creating your own `Patch` type
/// that implements [`Apply`] specifically for your needs.
///
/// This entire module can be disabled by setting `default-features` to `false`:
/// ```toml
/// someday = { version "0.0.0", default-features = false }
/// ```
pub mod patch;
#[cfg(feature = "patch")]
pub use patch::*;

pub use reader::Reader;
pub use commit::{Commit,CommitRef,CommitOwned};
pub use writer::Writer;
pub use apply::{Apply,ApplyReturn,ApplyReturnLt};

//---------------------------------------------------------------------------------------------------- Type alias.
/// An incrementing [`usize`] representing a new versions of data
///
/// In [`Commit`] objects, there is a [`Timestamp`] that represents that data's "version".
///
/// Unlike `git`, these aren't hashes in a [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree).
///
/// It is just an incrementing [`usize`].
///
/// Every time the [`Writer`] calls a commit operation like [`Writer::commit()`],
/// or [`Writer::overwrite()`] the data's [`Timestamp`] is incremented by `1`, thus
/// the timestamp is also how many commits there are.
///
/// An invariant that can be relied upon is that the [`Writer`] can
/// never "rebase" (as in, go back in time with their [`Commit`]) more
/// further back than the current [`Reader`]'s [`Timestamp`].
///
/// This means the [`Writer`]'s timestamp will _always_ be greater than or
/// equal to the [`Reader`]'s timestamp.
///
/// ## Example
/// ```rust
/// use someday::patch::PatchVec;
///
/// let v = vec![];
/// let (r, mut w) = someday::new(v);
///
/// // Writer writes some data, but does not commit.
/// w.add(PatchVec::Push("a"));
/// // Timestamp is still 0.
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(PatchVec::Push("b"));
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(PatchVec::Push("b"));
/// assert_eq!(w.timestamp(), 0);
///
/// // Now we commit.
/// w.commit();
/// assert_eq!(w.timestamp(), 1);
///
/// // We haven't pushed though, so
/// // readers will see timestamp of 0
/// assert_eq!(r.timestamp(), 0);
/// ```
pub type Timestamp = usize;

//---------------------------------------------------------------------------------------------------- Free functions
pub(crate) const INIT_VEC_LEN: usize = 16;

#[inline]
/// Create a new [`Writer`] & [`Reader`] pair
///
/// See their documentation for writing and reading functions.
///
/// This pre-allocates `16` capacity for the internal
/// [`Vec`]'s holding onto the `Patch`'s that have and
/// haven't been [`Apply`].
///
/// Use [`with_capacity()`] to set a custom capacity.
///
/// ## Example
/// ```rust
/// use someday::patch::PatchString;
///
/// let (r, mut w) = someday::new::<String, PatchString>("".into());
/// ```
pub fn new<T, Patch>(data: T) -> (Reader<T>, Writer<T, Patch>)
where
	T: Clone + Apply<Patch>,
{
	new_internal::<T, Patch>(data, INIT_VEC_LEN)
}

#[inline]
/// Create a new [`Writer`] & [`Reader`] pair with a specified [`Apply`] capacity
///
/// This is the same as [`crate::new()`] although the
/// the input `capacity` determines how much capacity the
/// [`Apply`] vectors will start out with.
///
/// Use this if you are planning to [`Writer::add()`]
/// many `Patch`'s before [`Writer::commit()`]'ing, so that
/// the internal [`Vec`]'s don't need to reallocate so often.
///
/// ## Example
/// ```rust
/// use someday::patch::PatchString;
///
/// // Can fit 128 patches without re-allocating.
/// let (r, mut w) = someday::with_capacity::<String, PatchString>("".into(), 128);
/// assert_eq!(w.staged().capacity(), 128);
/// assert_eq!(w.committed_patches().capacity(), 128);
/// ```
pub fn with_capacity<T, Patch>(data: T, capacity: usize) -> (Reader<T>, Writer<T, Patch>)
where
	T: Clone + Apply<Patch>,
{
	new_internal::<T, Patch>(data, capacity)
}

/// Create a default [`Writer`] & [`Reader`] pair
///
/// This is the same as [`crate::new()`] but it does not
/// require input data, it will generate your data using
/// [`Default::default()`].
///
/// ## Example
/// ```rust
/// use someday::patch::PatchString;
///
/// let (r, mut w) = someday::default::<String, PatchString>();
/// assert_eq!(*w.data(), "");
/// assert_eq!(r.head(), "");
/// ```
pub fn default<T, Patch>() -> (Reader<T>, Writer<T, Patch>)
where
	T: Default + Clone + Apply<Patch>,
{
	new_internal::<T, Patch>(Default::default(), INIT_VEC_LEN)
}

/// Create a default [`Writer`] & [`Reader`] pair with a specified [`Apply`] capacity
///
/// This is the same as [`crate::default`] combined with [`crate::with_capacity`].
///
/// ## Example
/// ```rust
/// use someday::patch::PatchString;
///
/// // Can fit 128 patches without re-allocating.
/// let (r, mut w) = someday::default_with_capacity::<String, PatchString>(128);
/// assert_eq!(w.staged().capacity(), 128);
/// assert_eq!(w.committed_patches().capacity(), 128);
/// ```
pub fn default_with_capacity<T, Patch>(capacity: usize) -> (Reader<T>, Writer<T, Patch>)
where
	T: Default + Clone + Apply<Patch>,
{
	new_internal::<T, Patch>(Default::default(), capacity)
}

fn new_internal<T, Patch>(data: T, capacity: usize) -> (Reader<T>, Writer<T, Patch>)
where
	T: Clone + Apply<Patch>,
{
	use std::sync::{Arc,atomic::AtomicBool};

	let local  = CommitOwned { timestamp: 0, data };
	let remote = Arc::new(local.clone());
	let arc    = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&remote)));
	let swapping = Arc::new(AtomicBool::new(false));

	let reader = Reader {
		arc: Arc::clone(&arc),
		swapping: Arc::clone(&swapping),
	};

	let writer = Writer {
		local: Some(local),
		remote,
		arc,
		patches: Vec::with_capacity(capacity),
		patches_old: Vec::with_capacity(capacity),
		tags: std::collections::BTreeMap::new(),
		swapping,
	};

	(reader, writer)
}