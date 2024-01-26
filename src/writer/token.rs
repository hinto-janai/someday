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
/// TODO
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
/// If this thread somehow panics in-between setting `writer_dead` to `false`
/// and returning the `Writer`, it'll be set to `false` forever which will
/// block (potential) future `Reader`'s from successfully calling this function.
///
/// In order to prevent this, create some drop clue to
/// set it to `true` if this function exits prematurely.
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