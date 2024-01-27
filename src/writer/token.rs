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
	patch::Patch,
	reader::Reader,
	commit::{CommitRef,CommitOwned,Commit},
	Timestamp,
	info::{
		CommitInfo,StatusInfo,
		PullInfo,PushInfo,WriterInfo,
	},
};

//---------------------------------------------------------------------------------------------------- Writer
#[derive(Clone, Debug)]
#[repr(transparent)]
/// Token representing a certain `Writer`, and if it has been dropped.
pub(crate) struct WriterToken {
	/// Only set to `false` when we are `drop()`'ed.
	inner: Arc<AtomicBool>,
}

impl WriterToken {
	/// If the `Writer` is dead, try reviving it.
	///
	/// If this returns `true`, if means the `Writer` is revived,
	/// and the caller has exclusive access, they can "become" the Writer.
	///
	/// Acquire + Relaxed ordering.
	pub(crate) fn try_revive(&self) -> Option<WriterReviveToken> {
		if self.inner.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) == Ok(true) {
			Some(WriterReviveToken {
				writer_token: self.clone(),
				dead: true,
			})
		} else {
			None
		}
	}

	#[must_use]
	/// TODO
	///
	/// Acquire ordering.
	pub(crate) fn is_dead(&self) -> bool {
		self.inner.load(Ordering::Acquire)
	}
}

impl From<Arc<AtomicBool>> for WriterToken {
	fn from(inner: Arc<AtomicBool>) -> Self {
		Self {
			inner,
		}
	}
}


impl Drop for WriterToken {
	fn drop(&mut self) {
		self.inner.store(true, Ordering::Release);
	}
}

//---------------------------------------------------------------------------------------------------- Writer trait impl
/// A token giving permission to become the new `Writer`.
///
/// If this token exists, it means:
/// 1. The previous `Writer` was dropped
/// 2. Thus, we have permission to "become" the `Writer`
///
/// This struct has drop-glue in-order to prevent it from
/// blocking other `Reader`'s who would like to become `Writer`'s
/// if a panic occurs, or if the "revive" function exits prematurely.
pub(crate) struct WriterReviveToken {
	/// The writer token.
	writer_token: WriterToken,
	/// If this is `true`, it will set the `Writer`
	/// to dead (false) on `drop()`, it must manually
	/// be set to `false` to avoid this.
	dead: bool,
}

impl WriterReviveToken {
	/// We successfully revived the `Writer`, no need to reset it to dead.
	pub(crate) fn revived(mut this: Self) {
		this.dead = false;
	}
}

impl Drop for WriterReviveToken {
	fn drop(&mut self) {
		self.writer_token.inner.store(self.dead, Ordering::Release);
	}
}