//---------------------------------------------------------------------------------------------------- Use
use crate::{Writer,Reader};

//---------------------------------------------------------------------------------------------------- Patch
/// The patches (functions) that will be applied to your data `T`
///
/// These are the patches (functions) that you use with
/// [`Writer::add`] that will modify your data, `T`.
///
/// The 2 inputs you can play with are:
/// - The [`Writer`]'s local mutable data, `T` (the thing you're modifying)
/// - The [`Reader`]'s latest head commit
///
/// This "patch" can either be a boxed dynamically dispatched function
/// (e.g, a capturing closure) or a regular function pointer.
///
/// By default, [`Patch::from`] will box functions, so:
/// ```rust
/// # use someday::*;
/// let (_, mut w) = someday::new::<i32>();
/// w.add(|w, _| {
/// 	*w = 123;
/// });
/// ```
/// will convert that closure into a `Box<dyn FnMut(&mut T, &T) + 'static + Send>)`
/// (assuming the compiler doesn't optimize out the allocation and dynamic dispatch).
///
/// If you have a non-capturing closure, you can make sure it isn't being boxed like so:
/// ```rust
/// # use someday::*;
/// let (_, mut w) = someday::new::<i32>();
/// w.add(Patch::Fn(|w, _| {
/// 	*w = 123;
/// }));
/// ```
pub enum Patch<T> {
	/// A heap allocated, dynamically dispatched function
	Box(Box<dyn FnMut(&mut T, &T) + 'static + Send>),
	/// A function pointer
	Fn(fn(writer: &mut T, reader: &T))
}

impl<T> Patch<T> {
	#[inline]
	// Apply the patch (function) to the writer data T.
	pub(crate) fn apply(&mut self, w: &mut T, r: &T) {
		match self {
			Self::Box(f) => f(w, r),
			Self::Fn(f) => f(w, r),
		}
	}
}

//---------------------------------------------------------------------------------------------------- Trait Impl
impl<T, F> From<F> for Patch<T>
where
	F: FnMut(&mut T, &T) + 'static + Send
{
	#[inline]
	fn from(patch: F) -> Self {
		Self::Box(Box::new(patch))
	}
}