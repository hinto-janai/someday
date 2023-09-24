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
mod snapshot;
mod writer;

/// Operations.
pub mod ops;

pub use reader::Reader;
pub use snapshot::{Snapshot,SnapshotOwned};
pub use writer::Writer;
pub use ops::Operation;

//---------------------------------------------------------------------------------------------------- Free functions
#[inline]
/// Create a new [`Writer`] & [`Reader`] pair
///
/// See their documentation for writing and reading functions.
///
/// This pre-allocates `24` capacity for the internal
/// [`Vec`] holding onto [`Operation`]'s that haven't been
/// [`Writer::commit()`]'ed yet.
///
/// Use [`with_capacity()`] to set a custom capacity.
pub fn new<T, O>(data: T) -> (Reader<T>, Writer<T, O>)
where
	T: Clone + Operation<O>,
{
	new_internal::<24, T, O>(data)
}

#[inline]
/// Create a new [`Writer`] & [`Reader`] pair with a specified [`Operation`] capacity
///
/// This is the same as [`new()`] although the
/// generic constant `N` determines how much capacity the
/// [`Operation`] vector will start out with.
///
/// Use this if you are planning to [`Writer::apply()`]
/// many operations before [`Writer::commit()`]'ing, so that
/// the internal [`Vec`] doesn't need to reallocate so often.
pub fn with_capacity<const N: usize, T, O>(data: T) -> (Reader<T>, Writer<T, O>)
where
	T: Clone + Operation<O>,
{
	new_internal::<N, T, O>(data)
}

pub(crate) fn new_internal<const N: usize, T, O>(data: T) -> (Reader<T>, Writer<T, O>)
where
	T: Clone + Operation<O>,
{
	use std::sync::Arc;

	let local = SnapshotOwned { timestamp: 0, data };
	// let dummy = SnapshotOwned { timestamp: 0, data: dummy };
	let now   = Arc::new(local.clone());
	let arc   = Arc::new(arc_swap::ArcSwapAny::new(Arc::clone(&now)));

	let reader = Reader {
		arc: Arc::clone(&arc),
	};

	let writer = Writer {
		local: Some(local),
		arc,
		now,
		ops: Vec::with_capacity(N),
	};

	(reader, writer)
}