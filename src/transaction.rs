//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
	borrow::{Borrow,BorrowMut},
	ops::{Deref,DerefMut},
};

use crate::{
	Timestamp,
	writer::Writer,
	patch::Patch,
	info::CommitInfo,
};

#[allow(unused_imports)] // docs
use std::sync::{Arc,Mutex};
#[allow(unused_imports)] // docs
use crate::Reader;

//---------------------------------------------------------------------------------------------------- Tx
/// Mutate the data `T` _directly_ without going through a [`Patch`].
///
/// This structure is returned by [`Writer::tx`], and can be seen as a
/// temporary handle for mutating your data `T`.
///
/// ## Mutable access to `T`
/// `Transaction` gives _direct_ mutable access to the `Writer`'s inner data `T` via:
/// - [`Transaction::data_mut`]
/// - [`Transaction::deref_mut`]
/// - [`Transaction::borrow_mut`]
/// - [`Transaction::as_mut`]
///
/// Each call to any of these functions will increment the [`Timestamp`]
/// by `1`, regardless if the data was changed or not.
///
/// ## [`Drop`]
/// After `Transaction` has been [`Transaction::commit()`]'ed or [`drop()`]'ed:
/// 1. All previous `Patch`'s get cleared
/// 2. A `Patch` that simply clones all the data gets added
///
/// It is worth noting that this only happens if a
/// mutable reference is created (even if it is not used).
///
/// ## `Transaction` vs `Patch`
/// Using `Transaction` instead of `Patch` when you are
/// just cloning data anyway may be preferred as it avoids:
/// - Potential use of [`Box`]/[`Arc`]
/// - Overhead of storing/re-applying `Patch`'s
///
/// `Patch`'s allow for turning cheaply reclaimed old
/// [`Reader`] data back into a viable copy, however, if your `Patch`
/// is simply cloning the data anyway, `Transaction` makes more sense.
///
/// ## Example
/// ```rust
/// # use someday::*;
/// // Create `Reader/Writer` pair of a `String`.
/// let (reader, mut writer) = someday::new(String::new());
///
/// // Open up a transaction.
/// let mut tx: Transaction<'_, String> = writer.tx();
/// // Each one of these is silently calling `deref_mut()`
/// // into the target `&mut String`, which increments the
/// // timestamp each call, so in total, 4 times.
/// tx.push_str("hello");
/// tx.push_str(" ");
/// tx.push_str("world");
/// tx.push_str("!");
///
/// // We started with a timestamp of 0,
/// // and after mutating a bit, are now at 4.
/// assert_eq!(tx.original_timestamp(), 0);
/// assert_eq!(tx.current_timestamp(), 4);
///
/// // Finish the transaction by dropping,
/// // we don't want the `CommitInfo` data
/// // from `Transaction::commit()`.
/// drop(tx);
/// // We can see dropping the `Transaction` added
/// // a `Patch` - this just clones the data.
/// assert_eq!(writer.staged().len(), 1);
///
/// // Our changes were applied
/// // to the `Writer` data directly.
/// assert_eq!(writer.data(), "hello world!");
/// assert_eq!(writer.timestamp(), 4);
/// // Although, we haven't `push()`'ed yet,
/// // so `Reader`'s still don't see changes.
/// assert_eq!(reader.head().data, "");
/// assert_eq!(reader.head().timestamp, 0);
///
/// // `Reader`'s can see our changes after `push()`.
/// writer.push();
/// assert_eq!(reader.head().data, "hello world!");
/// assert_eq!(reader.head().timestamp, 4);
/// ```
pub struct Transaction<'writer, T: Clone> {
	/// TODO
	pub(crate) writer: &'writer mut Writer<T>,
	/// TODO
	pub(crate) original_timestamp: Timestamp,
}

impl<'writer, T: Clone> Transaction<'writer, T> {
	/// TODO
	pub fn new(writer: &'writer mut Writer<T>) -> Transaction<'writer, T> {
		Self {
			original_timestamp: writer.timestamp(),
			writer,
		}
	}

	#[must_use]
	/// Immutably borrow the [`Writer`]'s data `T`.
	///
	/// This will not increment the [`Timestamp`].
	pub const fn data(&self) -> &T {
		// No need to increment timestamp,
		// the access cannot change `T`.
		&self.writer.local_as_ref().data
	}

	/// Mutably borrow the [`Writer`]'s data `T`.
	///
	/// Each call to this function will increment the [`Timestamp`]
	/// by `1`, regardless if the data was changed or not.
	///
	/// All mutable borrow trait functions
	/// use this function internally, which means
	/// they also increase the timestamp.
	pub fn data_mut(&mut self) -> &mut T {
		// Increment local timestamp assuming
		// each `deref_mut()` will actually mutate
		// the inner value.
		let commit = self.writer.local_as_mut();
		commit.timestamp += 1;

		&mut commit.data
	}

	#[must_use]
	/// TODO
	pub const fn original_timestamp(&self) -> Timestamp {
		self.original_timestamp
	}

	#[must_use]
	/// TODO
	pub const fn current_timestamp(&self) -> Timestamp {
		self.writer.timestamp()
	}

	#[must_use]
	/// TODO
	pub fn commit(self) -> CommitInfo {
		CommitInfo {
			patches: self.current_timestamp().saturating_sub(self.original_timestamp),
			timestamp_diff: self.current_timestamp().saturating_sub(self.writer.timestamp_remote()),
		}

		/* drop code */
	}

	/// TODO
	/// # Errors
	/// TODO
	pub fn abort(self) -> Result<(), Self> {
		if self.original_timestamp == self.current_timestamp() {
			Ok(())
		} else {
			Err(self)
		}
	}
}

//---------------------------------------------------------------------------------------------------- Drop
impl<T: Clone> Drop for Transaction<'_, T> {
	fn drop(&mut self) {
		// If we made changes, force a `clone` commit.
		if self.original_timestamp != self.current_timestamp() {
			// Clear old patches, they don't matter
			// anymore since we are cloning regardless.
			self.writer.patches.clear();
			self.writer.patches_old.clear();

			// Add a clone patch.
			self.writer.add(Patch::Ptr(|w, r| {
				*w = r.clone();
			}));
		}
	}
}

//---------------------------------------------------------------------------------------------------- Trait
impl<T: Clone> Deref for Transaction<'_, T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		self.writer
	}
}

impl<T: Clone> DerefMut for Transaction<'_, T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.data_mut()
	}
}

impl<T: Clone> Borrow<T> for Transaction<'_, T> {
	#[inline]
	fn borrow(&self) -> &T {
		self.data()
	}
}

impl<T: Clone> BorrowMut<T> for Transaction<'_, T> {
	#[inline]
	fn borrow_mut(&mut self) -> &mut T {
		self.data_mut()
	}
}

impl<T: Clone> AsRef<T> for Transaction<'_, T> {
	#[inline]
	fn as_ref(&self) -> &T {
		self.data()
	}
}

impl<T: Clone> AsMut<T> for Transaction<'_, T> {
	#[inline]
	fn as_mut(&mut self) -> &mut T {
		self.data_mut()
	}
}