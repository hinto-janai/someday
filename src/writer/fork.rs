//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;

use crate::{writer::token::WriterToken, writer::Writer};

#[allow(unused_imports)] // docs
use crate::{Patch, Reader};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    /// Fork off from the current [`Reader::head`] commit and create a [`Writer`].
    ///
    /// This new `Writer`:
    /// - will contain no [`Patch`]'s
    /// - is disconnected, meaning it has absolutely no
    /// relation to `self` or any other previous `Reader`'s.
    /// - has the latest [`Writer::head`] as the base for `Writer` and `Reader`'s
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new(String::new());
    ///
    /// // Connected `Reader` <-> `Writer`.
    /// assert!(r.connected_writer(&w));
    ///
    /// // Add local changes, but don't push.
    /// w.add_commit(|s, _| {
    ///     s.push_str("hello");
    /// });
    /// assert_eq!(w.data(), "hello");
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(r.head().data, "");
    /// assert_eq!(r.head().timestamp, 0);
    ///
    /// // Fork off into another `Writer`.
    /// let mut w2 = w.fork();
    /// let r2 = w2.reader();
    ///
    /// // It inherits the data of the previous `Writer`.
    /// assert_eq!(w.data(), "hello");
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.head().data, "hello");
    /// assert_eq!(w.head().timestamp, 1);
    ///
    /// // And has no relation to the previous `Writer/Reader`'s.
    /// assert!(!r.connected(&r2));
    /// assert!(!r.connected_writer(&w2));
    ///
    /// w2.add_commit(|s, _| {
    ///     s.push_str(" world!");
    /// });
    ///
    /// assert_eq!(w2.data(), "hello world!");
    /// assert_eq!(w2.timestamp(), 2);
    /// assert_eq!(w.data(), "hello");
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(r.head().data, "");
    /// assert_eq!(r.head().timestamp, 0);
    /// ```
    pub fn fork(&self) -> Self {
        let local = self.local.as_ref().unwrap().clone();
        let remote = Arc::new(local.clone());
        let arc = Arc::new(arc_swap::ArcSwap::new(Arc::clone(&remote)));

        Self {
            token: WriterToken::new(),
            local: Some(local),
            remote,
            arc,
            patches: Vec::with_capacity(self.patches.capacity()),
            patches_old: Vec::with_capacity(self.patches_old.capacity()),
        }
    }
}
