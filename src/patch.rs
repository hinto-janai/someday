//! Patch<T> (functions to apply to T)

//---------------------------------------------------------------------------------------------------- Use
#[allow(unused_imports)] // docs
use crate::{Writer,Reader};

//---------------------------------------------------------------------------------------------------- Patch
/// The patches (functions) that will be applied to your data `T`
///
/// These are the patches that you use with
/// [`Writer::add`] that will modify your data, `T`.
///
/// The 2 inputs you can play with are:
/// - The [`Writer`]'s local mutable data, `T` (the thing you're modifying)
/// - The [`Reader`]'s latest head commit
///
/// This "patch" can either be a boxed dynamically dispatched function
/// (e.g, a capturing closure) or a regular function pointer.
///
/// If you have a non-capturing closure, you can make sure it isn't being boxed like so:
/// ```rust
/// # use someday::*;
/// let (_, mut w) = someday::new::<i32>(0);
/// w.add(Patch::Fn(|w, _| {
///     *w = 123;
/// }));
/// ```
pub enum Patch<T> {
	#[allow(clippy::type_complexity)]
	/// A heap allocated, dynamically dispatched function
	Box(Box<dyn FnMut(&mut T, &T) + 'static + Send>),
	/// A function pointer
	Fn(fn(writer: &mut T, reader: &T))
}

impl<T> Patch<T> {
	#[inline]
	/// Short-hand for `Patch::Box(Box::new(f))`
	///
	/// ```rust
	/// # use someday::*;
	/// let (_, mut w) = someday::new::<i32>(0);
	///
	/// // These 2 are the exact same,
	/// // the 1st is just shorter.
	/// w.add(Patch::boxed(|w, _| {
	///     *w = 123;
	/// }));
	///
	/// w.add(Patch::Box(Box::new(|w, _| {
	///     *w = 123;
	/// })));
	/// ```
	pub fn boxed<F>(f: F) -> Self
	where
		F: FnMut(&mut T, &T) + 'static + Send
	{
		Self::Box(Box::new(f))
	}

	#[inline]
	/// Apply the patch (function) to the writer data T.
	pub(crate) fn apply(&mut self, w: &mut T, r: &T) {
		match self {
			Self::Box(f) => f(w, r),
			Self::Fn(f) => f(w, r),
		}
	}
}

//---------------------------------------------------------------------------------------------------- Trait Impl