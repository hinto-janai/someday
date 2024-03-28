//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::{sync::Arc, time::Duration};

use crate::{info::PushInfo, writer::Writer};

#[allow(unused_imports)] // docs
use crate::{Commit, Reader};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[inline]
    /// Conditionally push [`Writer`]'s local _committed_ data to the [`Reader`]'s.
    ///
    /// This will only push changes if there are new [`Commit`]'s
    /// (i.e if [`Writer::synced`] returns `true`).
    ///
    /// This may be expensive as there are other operations in this
    /// function (memory reclaiming, re-applying patches).
    ///
    /// This will return how many `Commit`'s the `Writer`'s pushed.
    ///
    /// `Reader`'s will atomically be able to access the
    /// the new `Commit` before this function is over.
    ///
    /// The `Patch`'s that were not [`commit()`](Writer::commit)'ed will not be
    /// pushed and will remain in the [`staged()`](Writer::staged) vector of patches.
    ///
    /// The [`PushInfo`] object returned is just a container
    /// for some metadata about the [`push()`](Writer::push) operation.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<String>("".into());
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    ///
    /// // This call does nothing since
    /// // we haven't committed anything.
    /// let push_info = w.push();
    /// assert_eq!(push_info.timestamp, 0);
    /// assert_eq!(push_info.commits, 0);
    /// assert_eq!(push_info.reclaimed, false);
    ///
    /// // Now there are commits to push.
    /// w.commit();
    ///
    /// if w.ahead() {
    ///     let push_info = w.push();
    ///     // We pushed 1 commit.
    ///     assert_eq!(push_info.timestamp, 1);
    ///     assert_eq!(push_info.commits, 1);
    ///     assert_eq!(push_info.reclaimed, true);
    /// } else {
    ///     // this branch cannot happen
    ///     unreachable!();
    /// }
    /// ```
    pub fn push(&mut self) -> PushInfo {
        self.push_inner::<false, ()>(None, None::<fn()>).0
    }

    #[inline]
    /// This function is the same as [`Writer::push()`]
    /// but it will [`std::thread::sleep()`] for at least `duration`
    /// amount of time to wait to reclaim the old [`Reader`]'s data.
    ///
    /// If `duration` has passed, the [`Writer`] will
    /// clone the data as normal and continue on.
    ///
    /// This is useful if you know your `Reader`'s only
    /// hold onto old data for a brief moment.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{sync::*,thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// # let barrier  = Arc::new(Barrier::new(2));
    /// # let other_b = barrier.clone();
    /// let commit = r.head();
    /// spawn(move || {
    ///     # other_b.wait();
    ///     // This `Reader` is holding onto the old data.
    ///     let moved = commit;
    ///     // But will let go after 1 millisecond.
    ///     sleep(Duration::from_millis(1));
    /// });
    ///
    /// # barrier.wait();
    /// // Wait 250 milliseconds before resorting to cloning data.
    /// let commit_info = w.push_wait(Duration::from_millis(250));
    /// // We pushed 1 commit.
    /// assert_eq!(commit_info.commits, 1);
    /// // And we successfully reclaimed the old data cheaply.
    /// assert_eq!(commit_info.reclaimed, true);
    /// ```
    pub fn push_wait(&mut self, duration: Duration) -> PushInfo {
        self.push_inner::<false, ()>(Some(duration), None::<fn()>).0
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// This function is the same as [`Writer::push()`]
    /// but it will execute the function `F` in the meanwhile before
    /// attempting to reclaim the old [`Reader`] data.
    ///
    /// The generic `R` is the return value of the function.
    /// Leaving it blank and having a non-returning function will
    /// be enough inference that the return value is `()`.
    ///
    /// Basically: "run the function `F` while we're waiting"
    ///
    /// This is useful to get some work done before waiting
    /// on the `Reader`'s to drop old copies of data.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{sync::*,thread::*,time::*,collections::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    ///
    /// # let barrier  = Arc::new(Barrier::new(2));
    /// # let other_b = barrier.clone();
    /// let head = r.head();
    /// spawn(move || {
    ///     # other_b.wait();
    ///     // This `Reader` is holding onto the old data.
    ///     let moved = head;
    ///     // But will let go after 100 milliseconds.
    ///     sleep(Duration::from_millis(100));
    /// });
    ///
    /// # barrier.wait();
    /// // Some work to be done.
    /// let mut hashmap = HashMap::<usize, String>::new();
    /// let mut vec     = vec![];
    ///
    /// // Commit.
    /// // Now the `Writer` is ahead by 1 commit, while
    /// // the `Reader` is hanging onto the old one.
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// // Pass in a closure, so that we can do
    /// // arbitrary things in the meanwhile...!
    /// let (push_info, return_value) = w.push_do(|| {
    ///     // While we're waiting, let's get some work done.
    ///     // Add a bunch of data to this HashMap.
    ///     (0..1_000).for_each(|i| {
    ///         hashmap.insert(i, format!("{i}"));
    ///     });
    ///     // Add some data to the vector.
    ///     (0..1_000).for_each(|_| {
    ///         vec.push(format!("aaaaaaaaaaaaaaaa"));
    ///     }); // <- `push_do()` returns `()`
    ///     # sleep(Duration::from_secs(1));
    /// });     // although we could return anything
    ///         // and it would be binded to `return_value`
    ///
    /// // At this point, the old `Reader`'s have
    /// // probably all dropped their old references
    /// // and we can probably cheaply reclaim our
    /// // old data back.
    ///
    /// // And yes, looks like we got it back cheaply:
    /// assert_eq!(push_info.reclaimed, true);
    ///
    /// // And we did some work
    /// // while waiting to get it:
    /// assert_eq!(hashmap.len(), 1_000);
    /// assert_eq!(vec.len(), 1_000);
    /// assert_eq!(return_value, ());
    /// ```
    pub fn push_do<F, R>(&mut self, f: F) -> (PushInfo, R)
    where
        F: FnOnce() -> R,
    {
        let (push_info, r) = self.push_inner::<false, R>(None, Some(f));

        // INVARIANT: we _know_ `R` will be a `Some`
        // because we provided a `Some`. `push_inner()`
        // will always return a Some(value).
        (push_info, r.unwrap())
    }

    #[inline]
    /// This function is the same as [`Writer::push()`]
    /// but it will **always** clone the data
    /// and not attempt to reclaim any old data.
    ///
    /// This is useful if you know reclaiming old data
    /// and re-applying your commits would take longer and/or
    /// be more expensive than cloning the data itself.
    ///
    /// Or if you know your `Reader`'s will be holding
    /// onto the data for a long time, and reclaiming data
    /// will be unlikely.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    /// w.add(Patch::Ptr(|w, _| w.push_str("abc")));
    /// w.commit();
    ///
    /// let commit = r.head();
    /// spawn(move || {
    ///     // This `Reader` will hold onto the old data forever.
    ///     let moved = commit;
    ///     loop { std::thread::park(); }
    /// });
    ///
    /// // Always clone data, don't wait.
    /// let push_info = w.push_clone();
    /// // We pushed 1 commit.
    /// assert_eq!(push_info.commits, 1);
    /// assert_eq!(push_info.reclaimed, false);
    /// ```
    pub fn push_clone(&mut self) -> PushInfo {
        self.push_inner::<true, ()>(None, None::<fn()>).0
    }

    /// Generic function to handle all the different types of pushes.
    fn push_inner<const CLONE: bool, R>(
        &mut self,
        duration: Option<Duration>,
        function: Option<impl FnOnce() -> R>,
    ) -> (PushInfo, Option<R>) {
        // Early return if no commits.
        if self.synced() {
            let return_value = function.map(|f| f());
            return (
                PushInfo {
                    timestamp: self.timestamp(),
                    commits: 0,
                    reclaimed: false,
                },
                return_value,
            );
        }

        // INVARIANT: we're temporarily "taking" our `self.local`.
        // It will be uninitialized for the time being.
        // We need to initialize it before returning.
        let local = self.local.take().unwrap();
        // Create the new `Reader` T.
        let new = Arc::new(local);

        // Update the `Reader` side with our new data.
        self.remote = Arc::clone(&new);
        let old = self.arc.swap(new);

        let timestamp_diff = self.remote.timestamp - old.timestamp;

        // Return early if the user wants to deep-clone no matter what.
        if CLONE {
            self.local = Some((*self.remote).clone());
            self.patches_old.clear();
            return (
                PushInfo {
                    timestamp: self.remote.timestamp,
                    commits: timestamp_diff,
                    reclaimed: false,
                },
                None,
            );
        }

        // If the user wants to execute a function
        // while waiting, do so and get the return value.
        let return_value = function.map(|f| f());

        // Try to reclaim data.
        let (mut local, reclaimed) = match Arc::try_unwrap(old) {
            // If there are no more dangling readers on the
            // old Arc we can cheaply reclaim the old data.
            Ok(old) => (old, true),

            // Else, if the user wants to
            // sleep and try again, do so.
            Err(old) => {
                if let Some(duration) = duration {
                    // Sleep.
                    std::thread::sleep(duration);
                    // Try again.
                    if let Some(old) = Arc::into_inner(old) {
                        (old, true)
                    } else {
                        ((*self.remote).clone(), false)
                    }
                } else {
                    // Else, there are dangling readers left.
                    // As to not wait on them, just expensively clone
                    // the inner data to have a mutually exclusive
                    // up-to-date local copy.
                    ((*self.remote).clone(), false)
                }
            }
        };

        if reclaimed {
            // Re-apply patches to this old data.
            for mut patch in self.patches_old.drain(..) {
                patch.apply(&mut local.data, &self.remote.data);
            }
            // Set proper timestamp if we're reusing old data.
            local.timestamp = self.remote.timestamp;
        } else {
            // Clear old patches.
            self.patches_old.clear();
        }

        // Re-initialize `self.local`.
        self.local = Some(local);

        // Output how many commits we pushed.
        (
            PushInfo {
                timestamp: self.remote.timestamp,
                commits: timestamp_diff,
                reclaimed,
            },
            return_value,
        )
    }
}
