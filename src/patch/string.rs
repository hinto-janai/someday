//---------------------------------------------------------------------------------------------------- use
use crate::{
	Apply,
	Writer,
	Reader,
};
use std::sync::Arc;

//---------------------------------------------------------------------------------------------------- PatchString
/// Common operations for [`String`]
///
/// The input [`str`]'s are wrapped in [`Arc`] for de-duplication.
///
/// ```rust
/// # use someday::*;
/// # use someday::patch::*;
/// // Create String.
/// let s = String::from("a");
/// let c = s.clone();
///
/// // Create Reader/Writer.
/// let (r, mut w) = someday::new(s);
///
/// // The readers see the data.
/// assert_eq!(r.head(), c);
/// assert_eq!(r.head().timestamp(), 0);
///
/// // Writer writes some data, but does not commit.
/// w.add(PatchString::PushStr("bc".into()));
/// // Writer/reader see old data still.
/// assert_eq!(w.data(), "a");
/// assert_eq!(r.head(), "a");
///
/// // Commit writes.
/// w.commit();
/// // Writer see changes locally.
/// assert_eq!(w.data(), "abc");
///
/// // Readers don't.
/// assert_eq!(r.head(), "a");
/// assert_eq!(r.head().timestamp(), 0);
///
/// // Writer pushes commits.
/// w.push();
/// // Now Readers see changes.
/// assert_eq!(r.head(), "abc");
/// assert_eq!(r.head().timestamp(), 1);
/// ```
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[non_exhaustive]
pub enum PatchString {
	/// [`String::clear`]
	Clear,
	/// [`String::insert_str`]
	InsertStr(usize, Arc<str>),
	/// [`String::push_str`]
	PushStr(Arc<str>),
	/// Assigns a new value to the [`String`]
	Set(Arc<str>),
}

impl Apply<PatchString> for String {
	fn apply(
		operation: &mut PatchString,
		writer: &mut Self,
		_reader: &Self,
	) {
		match operation {
			PatchString::Clear           => writer.clear(),
			PatchString::InsertStr(i, s) => writer.insert_str(*i, s),
			PatchString::PushStr(s)      => writer.push_str(&s),
			PatchString::Set(s)          => *writer = s.to_string(),
		}
	}
}