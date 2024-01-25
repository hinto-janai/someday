//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
	sync::{Arc,
		atomic::{
			AtomicBool,
			Ordering,
		},
	},
	time::Duration,
	borrow::Borrow,
	collections::BTreeMap,
	num::NonZeroUsize,
};

use crate::{
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Patch
/// TODO
pub enum Patch<T> {
	/// Dynamically dispatched, potentially capturing, boxed function.
	///
	/// ```rust
	/// let string = String::new();
	///
	/// let boxed: Box<dyn Fn()> = Box::new(move || {
	///     // The outside string was captured.
	///     println!("{string}");
	/// });
	///
	/// // This cannot be cloned.
	/// boxed();
	/// ```
	Box(Box<dyn Fn(&mut T, &T) + Send + 'static>),

	/// Dynamically dispatched, potentially capturing, cheaply clonable function.
	///
	/// ```rust
	/// # use std::sync::*;
	/// let string = String::new();
	///
	/// let arc: Arc<dyn Fn()> = Arc::new(move || {
	///     // The outside string was captured.
	///     println!("{string}");
	/// });
	///
	/// // We can clone this as much as we want though.
	/// let arc2 = Arc::clone(&arc);
	/// let arc3 = Arc::clone(&arc);
	/// arc();
	/// arc2();
	/// arc3();
	/// ```
	Arc(Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>),

	/// Non-capturing, static function pointer.
	///
	/// ```rust
	/// let ptr: fn() = || {
	///     // Nothing was captured.
	///     //
	///     // This closure can be coerced into
	///     // a function pointer, same as `fn()`.
	///     let string = String::new();
	///     println!("{string}");
	/// };
	///
	/// // Can copy it infinitely, it's just a pointer.
	/// let ptr2 = ptr;
	/// let ptr3 = ptr;
	/// ptr();
	/// ptr2();
	/// ptr3();
	/// ```
	Ptr(fn(&mut T, &T)),
}

impl<T> Patch<T> {
	#[inline]
	/// TODO
	pub fn boxed<P>(patch: P) -> Self
	where
		P: Fn(&mut T, &T) + Send + 'static,
	{
		Self::Box(Box::new(patch))
	}

	#[inline]
	/// TODO
	pub fn arc<P>(patch: Arc<P>) -> Self
	where
		P: Fn(&mut T, &T) + Send + Sync + 'static,
	{
		Self::Arc(patch)
	}

	#[inline]
	/// TODO
	pub fn ptr<P>(patch: fn(&mut T, &T)) -> Self {
		Self::Ptr(patch)
	}

	#[inline]
	/// TODO
	pub fn apply(&self, writer: &mut T, reader: &T) {
		match self {
			Self::Box(f) => f(writer, reader),
			Self::Arc(f) => f(writer, reader),
			Self::Ptr(f) => f(writer, reader),
		}
	}

	#[must_use]
	/// TODO
	pub const fn is_box(&self) -> bool {
		matches!(self, Self::Box(_))
	}

	#[must_use]
	/// TODO
	pub const fn is_arc(&self) -> bool {
		matches!(self, Self::Arc(_))
	}

	#[must_use]
	/// TODO
	pub const fn is_ptr(&self) -> bool {
		matches!(self, Self::Ptr(_))
	}
}

impl<T> From<Box<dyn Fn(&mut T, &T) + Send + 'static>> for Patch<T> {
	fn from(patch: Box<dyn Fn(&mut T, &T) + Send + 'static>) -> Self {
		Self::Box(patch)
	}
}

impl<T> From<Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>> for Patch<T> {
	fn from(patch: Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>) -> Self {
		Self::Arc(patch)
	}
}

impl<T> From<&Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>> for Patch<T> {
	fn from(patch: &Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>) -> Self {
		Self::Arc(Arc::clone(patch))
	}
}

impl<T> From<fn(&mut T, &T)> for Patch<T> {
	fn from(patch: fn(&mut T, &T)) -> Self {
		Self::Ptr(patch)
	}
}