use std::{ptr::NonNull, rc::Rc};

pub struct Owned;
pub struct Borrowed;

/// A smart pointer that have 2 states: owned and borrowed.
///
/// In compare to `Rc<RefCell<T>>` the `Data<T>` forbids having
/// multiple owners. Instead of it gives only a way to borrow data.
/// It's simple to understand that `Data` have only and only single
/// owner but a lot of borrowed data.
///
/// Also it may violate the semantic of borrowing and owning.
/// It means that you can have immutable and mutable values at the
/// same time, so be careful.
///
/// The `Data` doesn't implements `Send` + `Sync` traits due implementation.
pub struct Data<T, Status> {
    rc: Rc<NonNull<T>>,
    _status: Status,
}

impl<T> Data<T, Owned> {
    /// Creates a new `Data<T>` smart pointer.
    pub fn new(value: T) -> Self {
        Self {
            rc: Rc::new(unsafe { NonNull::new_unchecked(Box::into_raw(Box::new(value))) }),
            _status: Owned,
        }
    }

    /// Uses the value of owner and borrows it with creation a new `Data<T>`.
    pub fn borrow(&self) -> Data<T, Borrowed> {
        Data {
            rc: self.rc.clone(),
            _status: Borrowed,
        }
    }
}

impl<T> Clone for Data<T, Borrowed> {
    fn clone(&self) -> Self {
        Self {
            rc: self.rc.clone(),
            _status: Borrowed,
        }
    }
}

impl<T> std::ops::Deref for Data<T, Owned> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the pointer is valid because the `Data` owns value
        unsafe { self.rc.as_ref().as_ref() }
    }
}

impl<T> std::ops::DerefMut for Data<T, Owned> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the pointer and re-referencing are valid because the `Data` owns value
        unsafe { &mut *self.rc.as_ref().as_ptr() }
    }
}

impl<T> std::ops::Deref for Data<T, Borrowed> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the `Data` doesn't implements Send and Sync traits. Therefore there is no
        // implicit modification of data. Just borrowing as RefCell do.
        unsafe { self.rc.as_ref().as_ref() }
    }
}
