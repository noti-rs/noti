use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

/// An exclusive capability to access a value.
///
/// `Unique<T>` represents unique mutable access to a `T` whose ownership is
/// managed externally.
///
/// # Safety
///
/// The creator of `Unique` must guarantee that:
///
/// - the pointed-to value remains allocated for the entire lifetime of `Unique`;
/// - the pointed-to value is not accessed through another reference or pointer
///   while `Unique` provides access to it;
/// - the value is not moved while `Unique` exists;
/// - the value is not dropped while `Unique` exists;
/// - no second `Unique` referring to the same value is created.
pub struct Unique<T: ?Sized> {
    ptr: NonNull<T>,
    _marker: PhantomData<*mut T>,
}

impl<T: ?Sized> Unique<T> {
    /// Creates an exclusive capability from an exclusive mutable reference.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the pointee:
    ///
    /// - is valid and initialized as `T`;
    /// - remains allocated and at the same address for the entire lifetime of
    ///   the returned reference;
    /// - is not dropped while the returned reference exists;
    /// - is not accessed through any other reference or pointer while the
    ///   returned mutable reference exists;
    /// - is not moved while the returned reference exists.
    ///
    /// This method is intended for abstractions that maintain these guarantees
    /// through an external ownership and lifetime invariant.
    pub unsafe fn from_mut(data: &mut T) -> Self {
        Self {
            ptr: NonNull::from_mut(data),
            _marker: PhantomData,
        }
    }

    /// Converts this capability into a `'static` mutable reference.
    ///
    /// This operation erases the lifetime relationship between the returned
    /// reference and the allocation from which this capability was created.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the pointee:
    ///
    /// - is valid and initialized as `T`;
    /// - remains allocated and at the same address for the entire lifetime of
    ///   the returned reference;
    /// - is not dropped while the returned reference exists;
    /// - is not accessed through any other reference or pointer while the
    ///   returned mutable reference exists;
    /// - is not moved while the returned reference exists.
    ///
    /// In particular, because the returned reference has a `'static` lifetime,
    /// the caller must guarantee these conditions for as long as the reference
    /// can be used, not merely for the lifetime of this `Unique`.
    ///
    /// This method is intended for abstractions that maintain these guarantees
    /// through an external ownership and lifetime invariant.
    pub unsafe fn into_mut(&mut self) -> &'static mut T {
        self.ptr.as_mut()
    }
}

impl<T: ?Sized> Deref for Unique<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for Unique<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}
