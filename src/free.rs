//! Free functions.

//---------------------------------------------------------------------------------------------------- Use
use crate::{
	reader::Reader,
	writer::Writer,
	commit::CommitOwned,
};
use arc_swap::ArcSwapAny;
use std::{
	sync::{Arc,atomic::AtomicBool},
	collections::BTreeMap,
};

//---------------------------------------------------------------------------------------------------- Free functions
#[inline]
#[must_use]
/// Create a new [`Reader`] & [`Writer`] pair.
///
/// See their documentation for writing and reading functions.
///
/// ## Example
/// ```rust
/// let (reader, mut writer) = someday::new::<String>("hello world!".into());
///
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.data_remote(), "hello world!");
/// ```
pub fn new<T: Clone>(data: T) -> (Reader<T>, Writer<T>) {
	/// The default `Vec` capacity for the
	/// `Patch`'s when using using `new()`.
	const INIT_VEC_CAP: usize = 16;

	let local  = CommitOwned { timestamp: 0, data };
	let remote = Arc::new(local.clone());
	let arc    = Arc::new(ArcSwapAny::new(Arc::clone(&remote)));
	let swapping = Arc::new(AtomicBool::new(false));

	let writer = Writer {
		local: Some(local),
		remote,
		arc,
		patches: Vec::with_capacity(INIT_VEC_CAP),
		patches_old: Vec::with_capacity(INIT_VEC_CAP),
		tags: BTreeMap::new(),
		swapping,
	};

	(writer.reader(), writer)
}