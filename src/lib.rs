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

use std::collections::VecDeque;

pub use reader::Reader;
pub use snapshot::{Snapshot,SnapshotOwned};
pub use writer::Writer;
pub use ops::Operation;

//---------------------------------------------------------------------------------------------------- Free functions
#[inline]
///
pub fn new<T, O>(data: T) -> (Writer<T, O>, Reader<T>)
where
	T: Clone + Operation<O>,
{
	use std::sync::Arc;

	let local = SnapshotOwned { timestamp: 0, data };
	let now   = Arc::new(local.clone());
	let arc   = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&now)));

	let reader = Reader {
		arc: Arc::clone(&arc),
	};

	let writer = Writer {
		local,
		arc,
		now,
		ops: vec![],
	};

	(writer, reader)
}