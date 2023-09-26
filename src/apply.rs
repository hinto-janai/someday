///
pub trait Apply<Patch>
where
	Self: Clone,
{
	///
	fn apply(patch: &Patch, writer: &mut Self, reader: &Self);
}
