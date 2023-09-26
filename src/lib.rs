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

pub use reader::Reader;
pub use commit::{Commit,CommitRef,CommitOwned};
pub use writer::Writer;
pub use apply::Apply;

//---------------------------------------------------------------------------------------------------- Type alias.
/// A [`usize`] representing a "version" of data
///
/// In [`Commit`] and [`CommitOwned`], there will be a
/// [`Timestamp`] that represents that datas "version".
///
/// Unlike `git`, this isn't a [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree).
///
/// This is just an incrementing [`usize`].
///
/// Every time the [`Writer`] calls a commit operation like [`Writer::commit()`],
/// or [`Writer::overwrite()`] the data's [`Timestamp`] is incremented by `1`, thus
/// the timestamp is also how many commits there are.
///
/// An invariant that can be relied upon is that the [`Writer`] can
/// never "rebase", as in, go back in time with their [`Commit`]'s.
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
	new_internal::<INIT_VEC_LEN, T, Patch>(data)
}

#[inline]
/// Create a new [`Writer`] & [`Reader`] pair with a specified [`Apply`] capacity
///
/// This is the same as [`new()`] although the
/// generic constant `P` determines how much capacity the
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
/// let (r, mut w) = someday::with_capacity::<128, String, PatchString>("".into());
/// ```
pub fn with_capacity<const N: usize, T, Patch>(data: T) -> (Reader<T>, Writer<T, Patch>)
where
	T: Clone + Apply<Patch>,
{
	new_internal::<N, T, Patch>(data)
}

fn new_internal<const N: usize, T, Patch>(data: T) -> (Reader<T>, Writer<T, Patch>)
where
	T: Clone + Apply<Patch>,
{
	use std::sync::Arc;

	let local  = CommitOwned { timestamp: 0, data };
	let remote = Arc::new(local.clone());
	let arc    = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&remote)));

	let reader = Reader {
		arc: Arc::clone(&arc),
	};

	let writer = Writer {
		local: Some(local),
		remote,
		arc,
		patches: Vec::with_capacity(N),
		patches_old: Vec::with_capacity(N),
	};

	(reader, writer)
}