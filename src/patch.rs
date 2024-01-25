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
	writer::Writer,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Patch
/// Functions to be applied to your data `T`.
///
/// [`Patch`] is just a function that will be applied to your data `T`.
///
/// The [`Writer`] expects `T` modifications in the form of `Patch`'s.
///
/// The enumrated options are various forms of functions.
///
/// The 2 inputs you are given are:
/// - The `Writer`'s local mutable data, `T` (the thing you're modifying)
/// - The [`Reader`]'s latest head commit
///
/// ```rust
/// # use someday::*;
/// # use std::sync::*;
/// let (_, mut w) = someday::new::<String>("".into());
///
/// // Use a pre-defined function pointer.
/// fn fn_ptr(w: &mut String, r: &String) {
///     w.push_str("hello");
/// }
/// w.add(Patch::Ptr(fn_ptr));
///
/// // This non-capturing closure can also
/// // be coerced into a function pointer.
/// w.add(Patch::Ptr(|w, _| {
///     w.push_str("hello");
/// }));
///
/// // This capturing closure gets turned into
/// // a cheaply clone-able dynamic function.
/// let string = String::from("hello");
/// w.add(Patch::arc(move |w: &mut String ,_| {
///     let captured = &string;
///     w.push_str(&captured);
/// }));
/// ```
///
/// # ⚠️ Non-deterministic `Patch`
/// The `Patch`'s you use with [`Writer::add`] **must be deterministic**.
///
/// The `Writer` may apply your `Patch` twice, so any state that gets
/// modified or functions used in the `Patch` must result in the
/// same values as the first time the `Patch` was called.
///
/// Here is a **non-deterministic** example:
/// ```rust
/// # use someday::*;
/// # use std::sync::*;
/// static STATE: Mutex<usize> = Mutex::new(1);
///
/// let (_, mut w) = someday::new::<usize>(0);
///
/// w.add(Patch::boxed(move |w, _| {
///     let mut state = STATE.lock().unwrap();
///     *state *= 10; // 1*10 the first time, 10*10 the second time...
///     *w = *state;
/// }));
/// w.commit();
/// w.push();
///
/// // ⚠️⚠️⚠️ !!!
/// // The `Writer` reclaimed the old `Reader` data
/// // and applied our `Patch` again, except, the `Patch`
/// // was non-deterministic, so now the `Writer`
/// // and `Reader` have non-matching data...
/// assert_eq!(*w.data(), 100);
/// assert_eq!(*w.reader().head().data(), 10);
/// ```
pub enum Patch<T> {
	/// Dynamically dispatched, potentially capturing, boxed function.
	///
	/// ```rust
	/// let string = String::new();
	///
	/// let mut boxed: Box<dyn FnMut()> = Box::new(move || {
	///     // The outside string was captured.
	///     println!("{string}");
	/// });
	///
	/// // This cannot be cloned.
	/// boxed();
	/// ```
	Box(Box<dyn FnMut(&mut T, &T) + Send + 'static>),

	/// Dynamically dispatched, potentially capturing, cheaply [`Clone`]-able function.
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
	/// Short-hand for `Self::Box(Box::new(patch))`.
	pub fn boxed<P>(patch: P) -> Self
	where
		P: FnMut(&mut T, &T) + Send + 'static,
	{
		Self::Box(Box::new(patch))
	}

	#[inline]
	/// Short-hand for `Self::Arc(Arc::new(patch))`.
	pub fn arc<P>(patch: P) -> Self
	where
		P: Fn(&mut T, &T) + Send + Sync + 'static,
	{
		Self::Arc(Arc::new(patch))
	}

	#[inline]
	/// Apply the [`Patch`] onto the [`Writer`] data.
	pub(crate) fn apply(&mut self, writer: &mut T, reader: &T) {
		match self {
			Self::Box(f) => f(writer, reader),
			Self::Arc(f) => f(writer, reader),
			Self::Ptr(f) => f(writer, reader),
		}
	}

	#[must_use]
	/// If `self` is the `Patch::Box` variant.
	pub const fn is_box(&self) -> bool {
		matches!(self, Self::Box(_))
	}

	#[must_use]
	/// If `self` is the `Patch::Arc` variant.
	pub const fn is_arc(&self) -> bool {
		matches!(self, Self::Arc(_))
	}

	#[must_use]
	/// If `self` is the `Patch::Ptr` variant.
	pub const fn is_ptr(&self) -> bool {
		matches!(self, Self::Ptr(_))
	}
}

impl<T> From<Box<dyn FnMut(&mut T, &T) + Send + 'static>> for Patch<T> {
	fn from(patch: Box<dyn FnMut(&mut T, &T) + Send + 'static>) -> Self {
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