//---------------------------------------------------------------------------------------------------- Use

//---------------------------------------------------------------------------------------------------- Patch
/// TODO
pub enum Patch<T> {
	/// TODO
	Box(Box<dyn FnMut(&mut T, &T) + 'static + Send>),
	/// TODO
	Fn(fn(&mut T, &T))
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