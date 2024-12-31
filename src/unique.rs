//! Tools for using [`Rc`] and [`Arc`] to prove a stably-dereferencing level
//! of indirection in [`MutCursorRootedVec`](super::MutCursorRootedVec)'s API.
//!
//! While `Rc` and `Arc` are generally only offering *shared, immutable* access
//! to their interior, while [`MutCursorRootedVec`](super::MutCursorRootedVec) works
//! with *mutable* access, the standard library *does* offer a limited mutable-access
//! API in the form of the <code>{[Arc](Arc::get_mut), [Rc](Rc::get_mut)}::get_mut</code>
//! and <code>{[Arc](Arc::make_mut), [Rc](Rc::make_mut)}::make_mut</code> methods.
//!
//! These methods succeed when the reference-counted pointer is *unique*; in this
//! module, we provide an API to encode this uniqueness property into a separate type
//! which can fit the <code>[DerefMut] + [StableDeref]</code>
//! interface.

mod polyfill {
    use core::ffi::CStr;

    #[cfg(feature = "std")]
    use std::{ffi::OsStr, path::Path};

    use alloc::{rc::Rc, sync::Arc};
    /// Polyfill for unstable `CloneToUninit` in `std`.
    ///
    /// # SAFETY
    /// the `_make_mut` implementation must ensure uniqueness;
    /// after the method call without panicking, mutable access through`::as_ptr()` 
    /// must be sound, as long as the Rc/Arc is not cloned or otherwise shared.
    pub unsafe trait CloneToUninit {
        fn rc_make_mut(rc: &mut Rc<Self>);
        fn arc_make_mut(rc: &mut Arc<Self>);
    }

    macro_rules! impl_ {
        ($($({$($t:tt)*})? $Self:ty)*) => {$(
            unsafe impl$(<$($t)*>)? CloneToUninit for $Self {
                fn rc_make_mut(rc: &mut Rc<Self>) {
                    Rc::make_mut(rc);
                }
                fn arc_make_mut(rc: &mut Arc<Self>) {
                    Arc::make_mut(rc);
                }
            }
        )*};
    }
    impl_! {
        str CStr
        {T: Clone} [T]
        {T: Clone} T
    }
    #[cfg(feature = "std")]
    impl_! {
        OsStr Path
    }
}
#[cfg(docsrs)]
use core::clone::CloneToUninit;
// Use the *real* `CloneToUninit` for `docs.rs`, to make the `T: CloneToUninit` bounds below
// easier to understand for users (because this way the link becomes clickable).
#[cfg(not(docsrs))]
use polyfill::CloneToUninit;

use alloc::{rc::Rc, sync::Arc};
use stable_deref_trait::StableDeref;
use core::{
    ops::{Deref, DerefMut},
    panic::{RefUnwindSafe, UnwindSafe},
};

/// Extension trait for creating [`UniqueRc`]/[`UniqueArc`]
///
/// Take a look at the implementations below, to see the concrete type signatures
/// of these methods [for `Rc`](#impl-UniqueExt-for-Rc<T>) and [for `Arc`](#impl-UniqueExt-for-Arc<T>), respectively.
pub trait UniqueExt: Deref {
    type Unique;
    /// This function behaves like [`Rc::get_mut`]/[`Arc::get_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn get_unique(this: &mut Self) -> Option<&mut Self::Unique>;
    /// This function behaves like [`Rc::make_mut`]/[`Arc::make_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn make_unique(this: &mut Self) -> &mut Self::Unique
    where
        Self::Target: CloneToUninit;
}

impl<T: ?Sized> UniqueExt for Rc<T> {
    type Unique = UniqueRc<T>;

    /// This function behaves like [`Rc::get_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn get_unique(this: &mut Self) -> Option<&mut UniqueRc<T>> {
        Rc::get_mut(this)
            .is_some()
            .then(|| unsafe { &mut *(this as *mut Rc<T> as *mut UniqueRc<T>) })
    }

    /// This function behaves like [`Rc::make_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn make_unique(this: &mut Self) -> &mut UniqueRc<T>
    where
        T: CloneToUninit,
    {
        #[cfg(not(docsrs))]
        T::rc_make_mut(this);
        #[cfg(docsrs)]
        Rc::make_mut(this);
        unsafe { &mut *(this as *mut Rc<T> as *mut UniqueRc<T>) }
    }
}

impl<T: ?Sized> UniqueExt for Arc<T> {
    type Unique = UniqueArc<T>;

    /// This function behaves like [`Arc::get_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn get_unique(this: &mut Self) -> Option<&mut UniqueArc<T>> {
        Arc::get_mut(this)
            .is_some()
            .then(|| unsafe { &mut *(this as *mut Arc<T> as *mut UniqueArc<T>) })
    }

    /// This function behaves like [`Arc::make_mut`], but returns
    /// a reference that keeps the double-indirection necessary
    /// for use with [`MutCursorRootedVec`](super::MutCursorRootedVec).
    fn make_unique(this: &mut Self) -> &mut UniqueArc<T>
    where
        T: CloneToUninit,
    {
        #[cfg(not(docsrs))]
        T::arc_make_mut(this);
        #[cfg(docsrs)]
        Arc::make_mut(this);
        unsafe { &mut *(this as *mut Arc<T> as *mut UniqueArc<T>) }
    }
}

#[repr(transparent)]
pub struct UniqueRc<T: ?Sized>(Rc<T>);

#[repr(transparent)]
pub struct UniqueArc<T: ?Sized>(Arc<T>);

// adjust trait impls to match uniquely owned reference (compare e.g. `Box<T>`)

impl<T: ?Sized> UnwindSafe for UniqueRc<T> where T: UnwindSafe {}
impl<T: ?Sized> RefUnwindSafe for UniqueRc<T> where T: RefUnwindSafe {}

impl<T: ?Sized> UnwindSafe for UniqueArc<T> where T: UnwindSafe {}
impl<T: ?Sized> RefUnwindSafe for UniqueArc<T> where T: RefUnwindSafe {}

// yes, uniquely-owned Rc is completely thread-safe, too
unsafe impl<T: ?Sized> Send for UniqueRc<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for UniqueRc<T> where T: Sync {}

unsafe impl<T: ?Sized> Send for UniqueArc<T> where T: Send {}
unsafe impl<T: ?Sized> Sync for UniqueArc<T> where T: Sync {}

impl<T: ?Sized> Deref for UniqueRc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*Rc::as_ptr(&self.0) }
    }
}

impl<T: ?Sized> DerefMut for UniqueRc<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(Rc::as_ptr(&self.0) as *mut T) }
    }
}

unsafe impl<T: ?Sized> StableDeref for UniqueRc<T> {}

impl<T: ?Sized> Deref for UniqueArc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*Arc::as_ptr(&self.0) }
    }
}

impl<T: ?Sized> DerefMut for UniqueArc<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(Arc::as_ptr(&self.0) as *mut T) }
    }
}

unsafe impl<T: ?Sized> StableDeref for UniqueArc<T> {}
