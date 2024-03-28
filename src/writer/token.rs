//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

//---------------------------------------------------------------------------------------------------- Writer
#[derive(Clone, Debug)]
#[repr(transparent)]
/// Token representing a certain `Writer`, and if it has been dropped.
pub(crate) struct WriterToken {
    /// Is the `Writer` dead?
    ///
    /// Only set to `true` when we are `drop()`'ed.
    dead: Arc<AtomicBool>,
}

impl WriterToken {
    /// Return a new `Self` with a new `Arc(false)`.
    pub(crate) fn new() -> Self {
        Self {
            dead: Arc::new(AtomicBool::new(false)),
        }
    }

    /// If the `Writer` is dead, try reviving it.
    ///
    /// If this returns `true`, if means the `Writer` is revived,
    /// and the caller has exclusive access, they can "become" the Writer.
    ///
    /// Acquire + Relaxed ordering.
    pub(crate) fn try_revive(&self) -> Option<WriterReviveToken> {
        if self
            .dead
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            == Ok(true)
        {
            Some(WriterReviveToken::new(self))
        } else {
            None
        }
    }

    #[must_use]
    /// Is the `Writer` who held onto this token dead?
    ///
    /// Acquire ordering.
    pub(crate) fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Acquire)
    }
}

impl From<Arc<AtomicBool>> for WriterToken {
    fn from(dead: Arc<AtomicBool>) -> Self {
        Self { dead }
    }
}

impl Drop for WriterToken {
    fn drop(&mut self) {
        self.dead.store(true, Ordering::Release);
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
pub(crate) struct WriterReviveToken<'a> {
    /// The writer token.
    ///
    /// Must be borrowed such that we don't
    /// call its `drop()` when _we_ drop.
    writer_token: &'a WriterToken,
    /// If this is `true`, it will set the `Writer`
    /// to dead `drop()`, it must manually be set
    /// to `false` to avoid this.
    dead: bool,
}

impl<'a> WriterReviveToken<'a> {
    /// Attempt a revival, this must be "finished" by calling `Self::revived`.
    pub(crate) const fn new(writer_token: &'a WriterToken) -> WriterReviveToken<'a> {
        Self {
            writer_token,
            dead: true,
        }
    }

    /// We successfully revived the `Writer`, no need to reset it to dead.
    pub(crate) fn revived(mut this: Self) {
        this.dead = false;
    }
}

impl Drop for WriterReviveToken<'_> {
    fn drop(&mut self) {
        self.writer_token.dead.store(self.dead, Ordering::Release);
    }
}

//---------------------------------------------------------------------------------------------------- Tests
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    /// Assure token is set to `dead` set on drop.
    fn dead_on_drop() {
        let w = WriterToken::new();
        let r = w.clone();

        assert!(!r.is_dead());

        drop(w);
        assert!(r.is_dead());
    }

    #[test]
    /// Assure revival works.
    fn try_revive() {
        let w = WriterToken::new();
        let r = w.clone();

        assert!(r.try_revive().is_none());

        drop(w);
        assert!(r.is_dead());

        assert!(r.try_revive().is_some());
    }

    #[test]
    /// Assure the revival token sets state correctly after drop.
    fn revive_token() {
        let w = WriterToken::new();
        let r = w.clone();

        assert!(r.try_revive().is_none());

        assert!(!r.is_dead());
        drop(w);
        assert!(r.is_dead());

        let revive_token = r.try_revive().unwrap();
        assert!(!r.is_dead());
        // Should be set to automatically reset to `dead`
        // if we don't "complete" the revival.
        drop(revive_token);
        assert!(r.is_dead());

        // Try again, completing the revival.
        let revive_token = r.try_revive().unwrap();
        assert!(!r.is_dead());
        WriterReviveToken::revived(revive_token);
        assert!(!r.is_dead());
    }
}
