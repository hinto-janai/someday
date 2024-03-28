//! `Reader<T>` & `Writer<T>` commit timestamps.

//---------------------------------------------------------------------------------------------------- Use
#[allow(unused_imports)] // docs
use crate::{Commit, Reader, Writer};

//---------------------------------------------------------------------------------------------------- Timestamp
/// An incrementing [`usize`] representing a new versions of data.
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
/// # use someday::*;
/// let v = vec![];
/// let (r, mut w) = someday::new::<Vec<&str>>(v);
///
/// // Writer writes some data, but does not commit.
/// w.add(Patch::Ptr(|w, _| w.push("a")));
/// // Timestamp is still 0.
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(Patch::Ptr(|w, _| w.push("b")));
/// assert_eq!(w.timestamp(), 0);
///
/// w.add(Patch::Ptr(|w, _| w.push("b")));
/// assert_eq!(w.timestamp(), 0);
///
/// // Now we commit.
/// w.commit();
/// assert_eq!(w.timestamp(), 1);
///
/// // We haven't pushed though, so
/// // readers will see timestamp of 0
/// assert_eq!(r.head().timestamp, 0);
/// ```
pub type Timestamp = usize;
