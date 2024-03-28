//! `Writer<T>`

//---------------------------------------------------------------------------------------------------- Use
use std::sync::Arc;

#[allow(unused_imports)] // docs
use crate::{Reader, Writer};

//---------------------------------------------------------------------------------------------------- Patch
/// Functions to be applied to your data `T`.
///
/// [`Patch`] is just a function that will be applied to your data `T`.
///
/// The [`Writer`] expects `T` modifications in the form of `Patch`'s.
///
/// The enumerated options are various forms of functions.
///
/// The 2 inputs you are given are:
/// - The `Writer`'s local mutable data, `T` (the thing you're modifying)
/// - The [`Reader`]'s latest head commit
///
/// That is, `&mut T` is the `Writer` side, and `&T` is the `Reader`.
///
/// ```rust
/// # use someday::*;
/// # use std::sync::*;
/// let (_, mut w) = someday::new::<String>("".into());
///
/// // Use a pre-defined function pointer.
/// fn fn_ptr(w: &mut String, r: &String) {
///     w.push_str("hello");
/// }
/// w.add(Patch::Ptr(fn_ptr));
///
/// // This non-capturing closure can also
/// // be coerced into a function pointer.
/// w.add(Patch::Ptr(|w, _| {
///     w.push_str("hello");
/// }));
///
/// // This capturing closure gets turned into
/// // a cheaply clone-able dynamic function.
/// let string = String::from("hello");
/// w.add(Patch::arc(move |w: &mut String ,_| {
///     let captured = &string;
///     w.push_str(&captured);
/// }));
/// ```
///
/// # ⚠️ Non-deterministic `Patch`
/// The `Patch`'s you use with [`Writer::add`] **must be deterministic**.
///
/// The `Writer` may apply your `Patch` twice, so any state that gets
/// modified or functions used in the `Patch` must result in the
/// same values as the first time the `Patch` was called.
///
/// Here is a **non-deterministic** example:
/// ```rust
/// # use someday::*;
/// # use std::sync::*;
/// static STATE: Mutex<usize> = Mutex::new(1);
///
/// let (_, mut w) = someday::new::<usize>(0);
///
/// w.add(Patch::boxed(move |w, _| {
///     let mut state = STATE.lock().unwrap();
///     *state *= 10; // 1*10 the first time, 10*10 the second time...
///     *w = *state;
/// }));
/// w.commit();
/// w.push();
///
/// // ⚠️⚠️⚠️ !!!
/// // The `Writer` reclaimed the old `Reader` data
/// // and applied our `Patch` again, except, the `Patch`
/// // was non-deterministic, so now the `Writer`
/// // and `Reader` have non-matching data...
/// assert_eq!(*w.data(), 100);
/// assert_eq!(w.reader().head().data, 10);
/// ```
///
/// # The 2nd apply
/// Note that if/when the `Writer` applies your `Patch` for the 2nd time
/// inside [`Writer::push`], the `Reader` side of the data has _just_ been updated.
/// This means your `Patch`'s 2nd input `&T` will be referencing the _just_ pushed data.
///
/// ```rust
/// # use someday::*;
/// let (_, mut writer) = someday::new::<usize>(0);
///
/// writer.add(Patch::Ptr(|w, r| {
///     // `w` on the 1st apply of this `Patch`
///     // is our local `Writer` data. `r` is the
///     // current `Reader` data.
///     //
///     // The 2nd time this applies, `w` will be
///     // the old `Reader` data we are attempting
///     // to reclaim and "reproduce" with this `Patch`,
///     // while `r` will be the data the `Writer` just pushed.
/// }));
/// ```
pub enum Patch<T: Clone> {
    /// Dynamically dispatched, potentially capturing, boxed function.
    ///
    /// ```rust
    /// let string = String::new();
    ///
    /// let mut boxed: Box<dyn FnMut()> = Box::new(move || {
    ///     // The outside string was captured.
    ///     println!("{string}");
    /// });
    ///
    /// // This cannot be cloned.
    /// boxed();
    /// ```
    Box(Box<dyn FnMut(&mut T, &T) + Send + 'static>),

    /// Dynamically dispatched, potentially capturing, cheaply [`Clone`]-able function.
    ///
    /// ```rust
    /// # use std::sync::*;
    /// let string = String::new();
    ///
    /// let arc: Arc<dyn Fn()> = Arc::new(move || {
    ///     // The outside string was captured.
    ///     println!("{string}");
    /// });
    ///
    /// // We can clone this as much as we want though.
    /// let arc2 = Arc::clone(&arc);
    /// let arc3 = Arc::clone(&arc);
    /// arc();
    /// arc2();
    /// arc3();
    /// ```
    Arc(Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>),

    /// Non-capturing, static function pointer.
    ///
    /// ```rust
    /// let ptr: fn() = || {
    ///     // Nothing was captured.
    ///     //
    ///     // This closure can be coerced into
    ///     // a function pointer, same as `fn()`.
    ///     let string = String::new();
    ///     println!("{string}");
    /// };
    ///
    /// // Can copy it infinitely, it's just a pointer.
    /// let ptr2 = ptr;
    /// let ptr3 = ptr;
    /// ptr();
    /// ptr2();
    /// ptr3();
    /// ```
    Ptr(fn(&mut T, &T)),
}

impl<T: Clone + PartialEq> Patch<T> {
    /// A [`Patch::Ptr`] that clones the [`Reader`]'s data into
    /// the [`Writer`], but only if they are not [`PartialEq::eq`].
    pub const CLONE_IF_DIFF: Self = Self::Ptr(|w, r| {
        if w != r {
            *w = r.clone();
        }
    });
}

impl<T: Clone> Patch<T> {
    /// A [`Patch::Ptr`] that always clones the [`Reader`]'s data into the [`Writer`].
    pub const CLONE: Self = Self::Ptr(|w, r| *w = r.clone());
    /// A [`Patch::Ptr`] that does nothing.
    pub const NOTHING: Self = Self::Ptr(|_, _| {});

    #[inline]
    /// Short-hand for `Self::Box(Box::new(patch))`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let string = String::new();
    ///
    /// let boxed_patch = Patch::<String>::boxed(move |_, _| {
    ///     let captured_variable = &string;
    /// });
    /// assert!(boxed_patch.is_box());
    /// ```
    pub fn boxed<P>(patch: P) -> Self
    where
        P: FnMut(&mut T, &T) + Send + 'static,
    {
        Self::Box(Box::new(patch))
    }

    #[inline]
    /// Short-hand for `Self::Arc(Arc::new(patch))`.
    ///
    /// ```rust
    /// # use someday::*;
    /// let string = String::new();
    ///
    /// let arc_patch = Patch::<String>::arc(move |_, _| {
    ///     let captured_variable = &string;
    /// });
    /// assert!(arc_patch.is_arc());
    /// ```
    pub fn arc<P>(patch: P) -> Self
    where
        P: Fn(&mut T, &T) + Send + Sync + 'static,
    {
        Self::Arc(Arc::new(patch))
    }

    #[inline]
    /// Apply the [`Patch`] onto the [`Writer`] data.
    pub(crate) fn apply(&mut self, writer: &mut T, reader: &T) {
        match self {
            Self::Box(f) => f(writer, reader),
            Self::Arc(f) => f(writer, reader),
            Self::Ptr(f) => f(writer, reader),
        }
    }

    #[must_use]
    /// If `self` is the `Patch::Box` variant.
    pub const fn is_box(&self) -> bool {
        matches!(self, Self::Box(_))
    }

    #[must_use]
    /// If `self` is the `Patch::Arc` variant.
    pub const fn is_arc(&self) -> bool {
        matches!(self, Self::Arc(_))
    }

    #[must_use]
    /// If `self` is the `Patch::Ptr` variant.
    ///
    /// ```rust
    /// # use someday::*;
    /// let ptr_patch = Patch::<String>::Ptr(|w, _| {
    ///     // No captured variables, "pure" function.
    ///     w.push_str("hello");
    /// });
    /// assert!(ptr_patch.is_ptr());
    /// ```
    pub const fn is_ptr(&self) -> bool {
        matches!(self, Self::Ptr(_))
    }
}

impl<T: Clone> Default for Patch<T> {
    /// Returns [`Patch::NOTHING`].
    fn default() -> Self {
        Self::NOTHING
    }
}

impl<T: Clone> From<Box<dyn FnMut(&mut T, &T) + Send + 'static>> for Patch<T> {
    /// ```rust
    /// # use someday::*;
    /// let string = String::new();
    ///
    /// let boxed: Box<dyn FnMut(&mut String, &String) + Send + 'static> = Box::new(move |_, _| {
    ///     let captured_variable = &string;
    /// });
    ///
    /// let patch = Patch::from(boxed);
    /// assert!(patch.is_box());
    /// ```
    fn from(patch: Box<dyn FnMut(&mut T, &T) + Send + 'static>) -> Self {
        Self::Box(patch)
    }
}

impl<T: Clone> From<Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>> for Patch<T> {
    /// ```rust
    /// # use someday::*;
    /// # use std::sync::*;
    /// let string = String::new();
    ///
    /// let arc: Arc<dyn Fn(&mut String, &String) + Send + Sync + 'static> = Arc::new(move |_, _| {
    ///     let captured_variable = &string;
    /// });
    ///
    /// let patch = Patch::from(arc);
    /// assert!(patch.is_arc());
    /// ```
    fn from(patch: Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>) -> Self {
        Self::Arc(patch)
    }
}

impl<T: Clone> From<&Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>> for Patch<T> {
    /// ```rust
    /// # use someday::*;
    /// # use std::sync::*;
    /// let string = String::new();
    ///
    /// let arc: Arc<dyn Fn(&mut String, &String) + Send + Sync + 'static> = Arc::new(move |_, _| {
    ///     let captured_variable = &string;
    /// });
    ///
    /// let patch = Patch::from(&arc);
    /// assert!(patch.is_arc());
    /// ```
    fn from(patch: &Arc<dyn Fn(&mut T, &T) + Send + Sync + 'static>) -> Self {
        Self::Arc(Arc::clone(patch))
    }
}

impl<T: Clone> From<fn(&mut T, &T)> for Patch<T> {
    /// ```rust
    /// # use someday::*;
    /// let ptr: fn(&mut String, &String) = |w, _| {
    ///     w.push_str("hello");
    /// };
    ///
    /// let patch = Patch::from(ptr);
    /// assert!(patch.is_ptr());
    /// ```
    fn from(patch: fn(&mut T, &T)) -> Self {
        Self::Ptr(patch)
    }
}

impl<T: Clone> std::fmt::Debug for Patch<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Box(ptr) => {
                f.write_fmt(format_args!("Patch::Box({:?})", std::ptr::addr_of!(**ptr)))
            }
            Self::Arc(ptr) => {
                f.write_fmt(format_args!("Patch::Arc({:?})", std::ptr::addr_of!(**ptr)))
            }
            Self::Ptr(ptr) => f.write_fmt(format_args!("Patch::Ptr({ptr:?})")),
        }
    }
}
