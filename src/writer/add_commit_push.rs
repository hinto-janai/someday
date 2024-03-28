//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use crate::{
    info::{CommitInfo, PushInfo},
    patch::Patch,
    writer::Writer,
};
use std::time::Duration;

#[allow(unused_imports)] // docs
use crate::{Commit, Reader, Timestamp};

//---------------------------------------------------------------------------------------------------- Writer
impl<T: Clone> Writer<T> {
    #[inline]
    /// Add a [`Patch`] to apply to the data `T`
    ///
    /// This does not execute the `Patch` immediately,
    /// it will only store it for later usage.
    ///
    /// [`Commit`]-like operations are when these
    /// functions are applied to your data, e.g. [`Writer::commit()`].
    ///
    /// ```
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // Add a patch.
    /// w.add(Patch::Ptr(|w, _| *w += 1));
    ///
    /// // It hasn't been applied yet.
    /// assert_eq!(w.staged().len(), 1);
    ///
    /// // Now it has.
    /// w.commit();
    /// assert_eq!(w.staged().len(), 0);
    /// ```
    pub fn add(&mut self, patch: Patch<T>) {
        self.patches.push(patch);
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// Apply all the `Patch`'s that were [`add()`](Writer::add)'ed
    ///
    /// The new [`Commit`] created from this will become
    /// the `Writer`'s new [`Writer::head()`].
    ///
    /// You can `commit()` multiple times and it will
    /// only affect the `Writer`'s local data.
    ///
    /// You can choose when to publish those changes to
    /// the [`Reader`]'s with [`Writer::push()`].
    ///
    /// The [`CommitInfo`] object returned is just a container
    /// for some metadata about the `commit()` operation.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // Timestamp is 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // Add and commit a patch.
    /// w.add(Patch::Ptr(|w, _| *w += 123));
    /// w.commit();
    ///
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.head().data, 123);
    /// ```
    ///
    /// # Timestamp
    /// This will increment the [`Writer`]'s local [`Timestamp`] by `1`,
    /// but only if there were `Patch`'s to actually apply. In other
    /// words, if you did not call [`add()`](Writer::add) before this,
    /// [`commit()`](Writer::commit) will do nothing.
    ///
    /// ```rust
    /// # use someday::*;
    /// let (r, mut w) = someday::new::<usize>(0);
    ///
    /// // Timestamp is 0.
    /// assert_eq!(w.timestamp(), 0);
    ///
    /// // We didn't `add()` anything, but commit anyway.
    /// let commit_info = w.commit();
    /// assert_eq!(commit_info.patches, 0);
    /// assert_eq!(commit_info.timestamp_diff, 0);
    ///
    /// // There was nothing to commit,
    /// // so our timestamp did not change.
    /// assert_eq!(w.timestamp(), 0);
    /// assert_eq!(w.head().data, 0);
    /// ```
    pub fn commit(&mut self) -> CommitInfo {
        let patch_len = self.patches.len();

        // Early return if there was nothing to do.
        if patch_len == 0 {
            return CommitInfo {
                patches: 0,
                timestamp_diff: self.timestamp_diff(),
            };
        }

        self.local_as_mut().timestamp += 1;

        // Apply the patches and add to the old vector.
        //
        // Pre-allocate some space for the new patches.
        self.patches_old.reserve_exact(patch_len);

        for mut patch in self.patches.drain(..) {
            patch.apply(
                // We can't use `self.local_as_mut()` here
                // We can't have `&mut self` and `&self`.
                //
                // INVARIANT: local must be initialized after push()
                &mut self.local.as_mut().unwrap().data,
                &self.remote.data,
            );
            self.patches_old.push(patch);
        }

        CommitInfo {
            patches: patch_len,
            timestamp_diff: self.timestamp_diff(),
        }
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// [`add()`](Writer::add) and [`commit()`](Writer::commit)
    ///
    /// This function combines `add()` and `commit()` together.
    /// Since these actions are done together, a return value is allowed to be specified.
    ///
    /// This function will apply your [`Writer::staged()`] patches
    /// first, then your input `patch`, then `commit()` them.
    ///
    /// # Example
    /// If you'd like to receive a large chunk of data
    /// from your `T` instead of throwing it away:
    /// ```rust
    /// # use someday::*;
    /// // Very expensive data.
    /// let vec = (0..100_000).map(|i| format!("{i}")).collect();
    ///
    /// let (_, mut w) = someday::new::<Vec<String>>(vec);
    /// assert_eq!(w.timestamp(), 0);
    /// assert_eq!(w.timestamp_remote(), 0);
    ///
    /// // Add some patches normally.
    /// // These will be applied in `add_commit()` below.
    /// for i in 100_000..200_000 {
    ///     w.add(Patch::boxed(move |w: &mut Vec<String>, _| {
    ///         w.push(format!("{i}"));
    ///     }));
    /// }
    ///
    /// let (commit_info, r) = w.add_commit(|w, _| {
    ///     // Swap our value, and get back the strings.
    ///     // This implicitly becomes our <Output> (Vec<String>).
    ///     std::mem::take(w)
    /// });
    ///
    /// // We got our 200,000 `String`'s back
    /// // instead of dropping them!
    /// let r: Vec<String> = r;
    /// assert_eq!(r.len(), 200_000);
    /// assert_eq!(commit_info.patches, 100_001); // 100_000 normal patches + 1 `add_commit()`
    /// assert_eq!(commit_info.timestamp_diff, 1);
    ///
    /// // We got back our original strings.
    /// for (i, string) in r.into_iter().enumerate() {
    ///     assert_eq!(format!("{i}"), string);
    /// }
    ///
    /// // And the `Patch` got applied to the `Writer`'s data,
    /// // but hasn't been `push()`'ed yet.
    /// assert!(w.data().is_empty());
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.timestamp_remote(), 0);
    /// ```
    ///
    /// # Generics
    /// The generic inputs are:
    /// - `Patch`
    /// - `Output`
    ///
    /// `Patch` is the same as [`Writer::add()`] however, it has a
    /// `-> Output` value associated with it, this is defined by
    /// you, using the `Output` generic.
    ///
    /// # Timestamp
    /// This function will always increment the [`Writer`]'s local [`Timestamp`] by `1`.
    pub fn add_commit<P, Output>(&mut self, mut patch: P) -> (CommitInfo, Output)
    where
        P: FnMut(&mut T, &T) -> Output + Send + 'static,
    {
        // Commit the current patches.
        let mut commit_info = self.commit();

        // `commit()` won't update the timestamp
        // if there we no previous patches,
        // so make sure we do that.
        if commit_info.patches == 0 {
            self.local_as_mut().timestamp += 1;
            commit_info.timestamp_diff += 1;
        }
        // We're adding 1 more patch regardless.
        commit_info.patches += 1;

        // Commit the _input_ patch to our local data.
        let r = patch(&mut self.local.as_mut().unwrap().data, &self.remote.data);

        // Convert patch to immediately drop return value.
        self.patches_old
            .push(Patch::boxed(move |w, r| drop(patch(w, r))));

        (commit_info, r)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// [`add()`](Writer::add), [`commit()`](Writer::commit), and [`push()`](Writer::push)
    ///
    /// This function combines `add()`, `commit()`, `push()` together.
    /// Since these actions are done together, return values are allowed
    /// to be specified where they wouldn't otherwise be.
    ///
    /// The input `Patch` no longer has to be `Send + 'static` either.
    ///
    /// This allows you to specify any arbitrary return value from
    /// your `Patch`'s, even return values from your `T` itself.
    ///
    /// For example, if you'd like to receive a large chunk of data
    /// from your `T` instead of throwing it away:
    /// ```rust
    /// # use someday::*;
    /// // Very expensive data.
    /// let vec = (0..100_000).map(|i| format!("{i}")).collect();
    ///
    /// let (_, mut w) = someday::new::<Vec<String>>(vec);
    /// assert_eq!(w.timestamp(), 0);
    /// assert_eq!(w.timestamp_remote(), 0);
    ///
    /// let (info, r1, r2) = w.add_commit_push(|w, _| {
    ///     // Swap our value, and get back the strings.
    ///     // This implicitly becomes our <Output> (Vec<String>).
    ///     std::mem::take(w)
    /// });
    ///
    /// // We got our 100,000 `String`'s back
    /// // instead of dropping them!
    /// let r1: Vec<String> = r1;
    /// // If `Writer` reclaimed data and applied our
    /// // `Patch` to it, it also got returned!
    /// let r2: Option<Vec<String>> = r2;
    /// // And, some push info.
    /// let info: PushInfo = info;
    ///
    /// // We got back our original strings.
    /// for (i, string) in r1.into_iter().enumerate() {
    ///     assert_eq!(format!("{i}"), string);
    /// }
    ///
    /// // If the `Writer` reclaimed data,
    /// // then `r2` will _always_ be a `Some`.
    /// if info.reclaimed {
    ///     // This also contains 100,000 strings.
    ///     assert!(r2.is_some());
    /// }
    ///
    /// // And the `Patch` got applied to the `Writer`'s data.
    /// assert!(w.data().is_empty());
    /// assert_eq!(w.timestamp(), 1);
    /// assert_eq!(w.timestamp_remote(), 1);
    /// ```
    ///
    /// # Generics
    /// The generic inputs are:
    /// - `Patch`
    /// - `Output`
    ///
    /// `Patch` is the same as [`Writer::add()`] however, it has a
    /// `-> Output` value associated with it, this is defined by
    /// you, using the `Output` generic.
    ///
    /// # Returned Tuple
    /// The returned tuple is contains the regular [`PushInfo`]
    /// along with a `Output` and `Option<Output>`.
    ///
    /// The `Output` is the data returned by operating on the `Writer`'s side
    /// of the data.
    ///
    /// The `Option<Output>` is `Some` if the `Writer` reclaimed the `Reader`'s
    /// side of the data, and re-applied your `Patch` - it returns it instead
    /// of dropping it. This means that if `PushInfo`'s `reclaimed` is
    /// `true`, this `Option<Output>` will _always_ be `Some`.
    ///
    /// # Timestamp
    /// This function will always increment the [`Writer`]'s local [`Timestamp`] by `1`.
    pub fn add_commit_push<Patch, Output>(
        &mut self,
        patch: Patch,
    ) -> (PushInfo, Output, Option<Output>)
    where
        Patch: FnMut(&mut T, &T) -> Output,
    {
        let (push_info, return_1, return_2, _) =
            self.add_commit_push_inner::<false, Patch, Output, ()>(patch, None, None::<fn()>);
        (push_info, return_1, return_2)
    }

    #[inline]
    /// This is the same as [`Self::add_commit_push()`] with [`Self::push_wait()`] semantics.
    ///
    /// See `push_wait()`'s documentation for more info.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{sync::*,thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
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
    /// let (push_info, _, _) = w.add_commit_push_wait(
    ///     Duration::from_millis(250),
    ///     |w, _| w.push_str("abc"),
    /// );
    /// // We pushed 1 commit.
    /// assert_eq!(push_info.commits, 1);
    /// // And we successfully reclaimed the old data cheaply.
    /// assert_eq!(push_info.reclaimed, true);
    /// ```
    pub fn add_commit_push_wait<Patch, Output>(
        &mut self,
        duration: Duration,
        patch: Patch,
    ) -> (PushInfo, Output, Option<Output>)
    where
        Patch: FnMut(&mut T, &T) -> Output,
    {
        let (push_info, return_1, return_2, _) = self
            .add_commit_push_inner::<false, Patch, Output, ()>(patch, Some(duration), None::<fn()>);
        (push_info, return_1, return_2)
    }

    #[inline]
    #[allow(clippy::missing_panics_doc)]
    /// This is the same as [`Self::add_commit_push()`] with [`Self::push_do()`] semantics.
    ///
    /// See `push_do()`'s documentation for more info.
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
    /// // The actual `Patch` on our data.
    /// let patch = |w: &mut String, _: &_| w.push_str("abc");
    ///
    /// // The closure to do arbitrary things while pushing.
    /// let closure = || {
    ///     // While we're waiting, let's get some work done.
    ///     // Add a bunch of data to this HashMap.
    ///     (0..1_000).for_each(|i| {
    ///         hashmap.insert(i, format!("{i}"));
    ///     });
    ///     // Add some data to the vector.
    ///     (0..1_000).for_each(|_| {
    ///         vec.push(format!("aaaaaaaaaaaaaaaa"));
    ///     }); // <- `add_commit_push_do()` returns `()`
    ///     # sleep(Duration::from_secs(1));
    /// };      // although we could return anything
    ///         // and it would be binded to `return_value`
    ///
    /// // Pass in a closure, so that we can do
    /// // arbitrary things in the meanwhile...!
    /// let (push_info, _, _, return_value) = w.add_commit_push_do(closure, patch);
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
    pub fn add_commit_push_do<Patch, Output, F, R>(
        &mut self,
        f: F,
        patch: Patch,
    ) -> (PushInfo, Output, Option<Output>, R)
    where
        Patch: FnMut(&mut T, &T) -> Output,
        F: FnOnce() -> R,
    {
        let (push_info, return_1, return_2, r) =
            self.add_commit_push_inner::<false, Patch, Output, R>(patch, None, Some(f));
        // INVARIANT: we _know_ `R` will be a `Some`
        // because we provided a `Some`. `add_commit_push_inner()`
        // will always return a Some(value).
        (push_info, return_1, return_2, r.unwrap())
    }

    #[inline]
    /// This is the same as [`Self::add_commit_push()`] with [`Self::push_clone()`] semantics.
    ///
    /// See `push_clone()`'s documentation for more info.
    ///
    /// ```rust
    /// # use someday::*;
    /// # use std::{thread::*,time::*};
    /// let (r, mut w) = someday::new::<String>("".into());
    /// let (push_info, _, _) = w.add_commit_push_clone(|w, _| {
    ///     w.push_str("abc");
    /// });
    ///
    /// assert_eq!(push_info.commits, 1);
    /// assert_eq!(push_info.reclaimed, false);
    /// ```
    pub fn add_commit_push_clone<Patch, Output>(
        &mut self,
        patch: Patch,
    ) -> (PushInfo, Output, Option<Output>)
    where
        Patch: FnMut(&mut T, &T) -> Output,
    {
        let (push_info, return_1, return_2, _) =
            self.add_commit_push_inner::<true, Patch, Output, ()>(patch, None, None::<fn()>);
        (push_info, return_1, return_2)
    }

    /// Generic function to handle all the different types of `add_commit_push`'s.
    fn add_commit_push_inner<const CLONE: bool, Patch, Output, R>(
        &mut self,
        mut patch: Patch,
        duration: Option<Duration>,
        function: Option<impl FnOnce() -> R>,
    ) -> (PushInfo, Output, Option<Output>, Option<R>)
    where
        // We're never storing this `Patch` so it
        // doesn't have to be `Send + 'static`.
        Patch: FnMut(&mut T, &T) -> Output,
    {
        // Commit `Patch` to our local data.
        self.local_as_mut().timestamp += 1;
        let return_1 = patch(&mut self.local.as_mut().unwrap().data, &self.remote.data);

        // Push all commits so far.
        let (push_info, r) = if CLONE {
            (self.push_clone(), None)
        } else if let Some(duration) = duration {
            (self.push_wait(duration), None)
        } else if let Some(function) = function {
            let (push_info, r) = self.push_do(function);
            (push_info, Some(r))
        } else {
            (self.push(), None)
        };

        // If the `Writer` reclaimed data, we must re-apply
        // since we did not push the Patch onto the `patches_old` Vec
        // (since we want the return value).
        let return_2 = (!CLONE && push_info.reclaimed)
            .then(|| patch(&mut self.local.as_mut().unwrap().data, &self.remote.data));

        (push_info, return_1, return_2, r)
    }
}
