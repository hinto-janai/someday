///
pub trait Operation<O> {
	///
	fn apply(&mut self, operation: &mut O);
}