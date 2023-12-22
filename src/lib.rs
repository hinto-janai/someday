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
	unsafe_code,
)]
#![deny(
	clippy::all,
	clippy::correctness,
	clippy::suspicious,
	clippy::style,
	clippy::complexity,
	clippy::perf,
	clippy::pedantic,
	clippy::restriction,
	clippy::nursery,
	clippy::cargo,
	unused_comparisons,
	nonstandard_style,
)]
#![allow(
	clippy::single_char_lifetime_names,
	clippy::implicit_return,
	clippy::std_instead_of_alloc,
	clippy::std_instead_of_core,
	clippy::unwrap_used,
	clippy::min_ident_chars,
	clippy::absolute_paths,
	clippy::missing_inline_in_public_items,
	clippy::arithmetic_side_effects,
	clippy::unwrap_in_result,
	clippy::pattern_type_mismatch,
	clippy::shadow_reuse,
	clippy::shadow_unrelated,
	clippy::missing_trait_methods,
	clippy::pub_use,
	clippy::pub_with_shorthand,
	clippy::blanket_clippy_restriction_lints,
	clippy::exhaustive_structs,
	clippy::exhaustive_enums,
)]

//---------------------------------------------------------------------------------------------------- Mod
pub mod commit;
pub use commit::*;

pub mod info;
pub use info::*;

mod reader;
pub use reader::Reader;

mod patch;
pub use patch::Patch;

mod writer;
pub use writer::Writer;

//---------------------------------------------------------------------------------------------------- Type alias.
/// An incrementing [`usize`] representing a new versions of data
///
/// In [`Commit`] objects, there is a [`Timestamp`] that represents that data's "version".
///
/// It is just an incrementing [`usize`] starting at 0.
///
/// Every time the [`Writer`] calls a commit operation like [`Writer::commit()`],
/// or [`Writer::overwrite()`] the data's [`Timestamp`] is incremented by `1`, thus
/// the timestamp is also how many commits there are.
///
/// An invariant that can be relied upon is that the [`Writer`] can
/// never "rebase" (as in, go back in time with their [`Commit`]) more
/// further back than the current [`Reader`]'s [`Timestamp`].
///
/// This means the [`Writer`]'s timestamp will _always_ be
/// greater than or equal to the [`Reader`]'s timestamp.
///
/// ## Example
/// ```rust
/// use someday::Patch;
///
/// let v = vec![];
/// let (r, mut w) = someday::new::<Vec<&str>>(v);
///
/// // Writer writes some data, but does not commit.
/// w.add(Patch::Fn(|w, _| w.push("a")));
/// // Timestamp is still 0.
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(Patch::Fn(|w, _| w.push("b")));
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(Patch::Fn(|w, _| w.push("b")));
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
/// The default `Vec` capacity for the
/// `Patch`'s when using using `new()`.
pub(crate) const INIT_VEC_LEN: usize = 16;

#[inline]
#[must_use]
/// Create a new [`Writer`] & [`Reader`] pair
///
/// See their documentation for writing and reading functions.
///
/// This pre-allocates `16` capacity for the internal
/// [`Vec`]'s holding onto the [`Patch`]'s that have and
/// haven't been applied.
///
/// Use [`with_capacity()`] to set a custom capacity.
///
/// ## Example
/// ```rust
/// let (reader, mut writer) = someday::new::<String>("".into());
/// ```
pub fn new<T>(data: T) -> (Reader<T>, Writer<T>)
where
	T: Clone,
{
	new_internal::<T>(data, INIT_VEC_LEN)
}

#[inline]
#[must_use]
/// Create a new [`Writer`] & [`Reader`] pair with a specified [`Patch`] capacity
///
/// This is the same as [`crate::new()`] although the
/// the input `capacity` determines how much capacity the
/// [`Patch`] vectors will start out with.
///
/// Use this if you are planning to [`Writer::add()`]
/// many [`Patch`]'s before [`Writer::commit()`]'ing, so that
/// the internal [`Vec`]'s don't need to reallocate so often.
///
/// ## Example
/// ```rust
/// // Can fit 128 functions without re-allocating.
/// let (r, mut w) = someday::new_with_capacity::<String>("".into(), 128);
/// assert_eq!(w.staged().capacity(), 128);
/// assert_eq!(w.committed_patches().capacity(), 128);
/// ```
pub fn new_with_capacity<T>(data: T, capacity: usize) -> (Reader<T>, Writer<T>)
where
	T: Clone,
{
	new_internal::<T>(data, capacity)
}

#[inline]
#[must_use]
/// Create a default [`Writer`] & [`Reader`] pair
///
/// This is the same as [`crate::new()`] but it does not
/// require input data, it will generate your data using
/// [`Default::default()`].
///
/// ## Example
/// ```rust
/// let (r, mut w) = someday::default::<String>();
/// assert_eq!(*w.data(), "");
/// assert_eq!(r.head(), "");
/// ```
pub fn default<T>() -> (Reader<T>, Writer<T>)
where
	T: Default + Clone,
{
	new_internal::<T>(Default::default(), INIT_VEC_LEN)
}

#[inline]
#[must_use]
/// Create a default [`Writer`] & [`Reader`] pair with a specified [`Patch`] capacity
///
/// This is the same as [`crate::default`] combined with [`crate::with_capacity`].
///
/// ## Example
/// ```rust
/// // Can fit 128 functions without re-allocating.
/// let (r, mut w) = someday::default_with_capacity::<String>(128);
/// assert_eq!(w.staged().capacity(), 128);
/// assert_eq!(w.committed_patches().capacity(), 128);
/// ```
pub fn default_with_capacity<T>(capacity: usize) -> (Reader<T>, Writer<T>)
where
	T: Default + Clone,
{
	new_internal::<T>(Default::default(), capacity)
}

/// Internal generic functions used by all `new()` functions above.
fn new_internal<T>(data: T, capacity: usize) -> (Reader<T>, Writer<T>)
where
	T: Clone,
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