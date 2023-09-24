//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;

//---------------------------------------------------------------------------------------------------- SnapshotOwned
#[derive(Clone)]
#[repr(C)]
/// Container for the timestamp and data.
///
/// Could just be (u64, T) as well
/// but the fields make it more clear.
pub struct SnapshotOwned<T>
where
	T: Sized,
{
	/// Timestamp.
	/// Increments by 1 every time
	/// a writer's `update()` is called.
	pub timestamp: usize,

	/// The generic data `T`.
	pub data: T,
}

//---------------------------------------------------------------------------------------------------- SnapshotOwned Trait
impl<T> TryFrom<Snapshot<T>> for SnapshotOwned<T> {
	type Error = Snapshot<T>;

	fn try_from(snapshot: Snapshot<T>) -> Result<Self, Self::Error> {
		Arc::try_unwrap(snapshot.inner).map_err(|inner| Snapshot { inner })
	}
}

impl<T> std::ops::Deref for SnapshotOwned<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<T> PartialEq for SnapshotOwned<T> {
	fn eq(&self, other: &Self) -> bool {
		// INVARIANT: We always update the timestamp
		// if the data is different, so if the timestamp
		// is the same, the data should be the same.
		self.timestamp == other.timestamp
	}
}

impl<T> PartialEq<T> for SnapshotOwned<T>
where
	T: PartialEq<T>,
{
	fn eq(&self, other: &T) -> bool {
		self.data == *other
	}
}

impl<T> PartialOrd for SnapshotOwned<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		// INVARIANT: same as PartialEq.
		self.timestamp.partial_cmp(&self.timestamp)
	}
}

impl<T> PartialOrd<T> for SnapshotOwned<T>
where
	T: PartialOrd<T>,
{
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		// INVARIANT: same as PartialEq.
		self.data.partial_cmp(&other)
	}
}

impl<T> AsRef<T> for SnapshotOwned<T> {
	fn as_ref(&self) -> &T {
		&self.data
	}
}

impl<T> std::borrow::Borrow<T> for SnapshotOwned<T> {
	fn borrow(&self) -> &T {
		&self.data
	}
}

impl<T> std::fmt::Display for SnapshotOwned<T>
where
	T: std::fmt::Display,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.data, f)
	}
}

impl<T> std::fmt::Debug for SnapshotOwned<T>
where
	T: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SnapshotOwned")
			.field("timestamp", &self.timestamp)
			.field("data", &self.data)
			.finish()
	}
}

//---------------------------------------------------------------------------------------------------- Snapshot
#[derive(Clone)]
///
pub struct Snapshot<T> {
	pub(super) inner: Arc<SnapshotOwned<T>>,
}

impl<T> Snapshot<T> {
	#[inline]
	///
	pub fn timestamp(&self) -> usize {
		self.inner.timestamp
	}

	#[inline]
	///
	pub fn data(&self) -> &T {
		&self.inner.data
	}

	///
	pub fn count(&self) -> usize {
		Arc::strong_count(&self.inner)
	}

	///
	fn to_owned(&self) -> SnapshotOwned<T> where T: Clone {
		SnapshotOwned {
			timestamp: self.inner.timestamp,
			data: self.inner.data.clone(),
		}
	}

	///
	fn into_owned(self) -> SnapshotOwned<T> where T: Clone {
		match Arc::try_unwrap(self.inner) {
			Ok(s) => s,
			Err(s) => SnapshotOwned {
				timestamp: s.timestamp,
				data: s.data.clone(),
			}
		}
	}
}

//---------------------------------------------------------------------------------------------------- Snapshot Trait impl
impl<T> std::ops::Deref for Snapshot<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.inner.data
	}
}

impl<T> PartialEq for Snapshot<T> {
	fn eq(&self, other: &Self) -> bool {
		self.inner == other.inner
	}
}

impl<T> PartialEq<T> for Snapshot<T>
where
	T: PartialEq<T>,
{
	fn eq(&self, other: &T) -> bool {
		self.inner.data == *other
	}
}

impl<T> PartialOrd for Snapshot<T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.inner.partial_cmp(&other.inner)
	}
}

impl<T> PartialOrd<T> for Snapshot<T>
where
	T: PartialOrd<T>,
{
	fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
		self.inner.data.partial_cmp(&other)
	}
}

impl<T> AsRef<T> for Snapshot<T> {
	fn as_ref(&self) -> &T {
		&self.inner.data
	}
}

impl<T> std::borrow::Borrow<T> for Snapshot<T> {
	fn borrow(&self) -> &T {
		&self.inner.data
	}
}

impl<T> std::fmt::Display for Snapshot<T>
where
	T: std::fmt::Display,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(&self.inner.data, f)
	}
}

impl<T> std::fmt::Debug for Snapshot<T>
where
	T: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Snapshot")
			.field("timestamp", &self.inner.timestamp)
			.field("data", &self.inner.data)
			.finish()
	}
}
