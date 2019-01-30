#![unstable(feature = "futures_api",
            reason = "futures in libcore are unstable",
            issue = "50547")]

use fmt;
use marker::Unpin;

/// A `RawWaker` allows the implementor of a task executor to create a `Waker`
/// which provides customized wakeup behavior.
///
/// It consists of a data pointer and a virtual function pointer table (vtable) that
/// customizes the behavior of the `RawWaker`.
#[derive(PartialEq)]
pub struct RawWaker {
    /// A data pointer, which can be used to store arbitrary data as required
    /// by the executor. This could be e.g. a type-erased pointer to an `Arc`
    /// that is associated with the task.
    /// The value of this field gets passed to all functions that are part of
    /// the vtable as first parameter.
    pub data: *const (),
    /// Virtual function pointer table that customizes the behavior of this waker.
    pub vtable: &'static RawWakerVTable,
}

impl fmt::Debug for RawWaker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RawWaker")
            .finish()
    }
}

/// A virtual function pointer table (vtable) that specifies the behavior
/// of a `RawWaker`.
///
/// The pointer passed to all functions inside the vtable is the `data` pointer
/// from the enclosing `RawWaker` object.
#[derive(PartialEq, Copy, Clone)]
pub struct RawWakerVTable {
    /// This function will be called when the `RawWaker` gets cloned, e.g. when
    /// the `Waker` in which the `RawWaker` is stored gets cloned.
    ///
    /// The implementation of this function must retain all resources that are
    /// required for this additional instance of a `RawWaker` and associated
    /// task. Calling `wake` on the resulting `RawWaker` should result in a wakeup
    /// of the same task that would have been awoken by the original `RawWaker`.
    pub clone: unsafe fn(*const ()) -> RawWaker,

    /// This function will be called when `wake` is called on the `Waker`.
    /// It must wake up the task associated with this `RawWaker`.
    pub wake: unsafe fn(*const ()),

    /// This function gets called when a `RawWaker` gets dropped.
    ///
    /// The implementation of this function must make sure to release any
    /// resources that are associated with this instance of a `RawWaker` and
    /// associated task.
    pub drop_fn: unsafe fn(*const ()),
}

impl fmt::Debug for RawWakerVTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RawWakerVTable")
            .finish()
    }
}

/// A `Waker` is a handle for waking up a task by notifying its executor that it
/// is ready to be run.
///
/// This handle encapsulates a `RawWaker` instance, which defines the
/// executor-specific wakeup behavior.
///
/// Implements `Clone`, `Send`, and `Sync`.
#[repr(transparent)]
pub struct Waker {
    waker: RawWaker,
}

impl Unpin for Waker {}
unsafe impl Send for Waker {}
unsafe impl Sync for Waker {}

impl Waker {
    /// Wake up the task associated with this `Waker`.
    pub fn wake(&self) {
        // The actual wakeup call is delegated through a virtual function call
        // to the implementation which is defined by the executor.
        unsafe { (self.waker.vtable.wake)(self.waker.data) }
    }

    /// Returns whether or not this `Waker` and other `Waker` have awaken the same task.
    ///
    /// This function works on a best-effort basis, and may return false even
    /// when the `Waker`s would awaken the same task. However, if this function
    /// returns `true`, it is guaranteed that the `Waker`s will awaken the same task.
    ///
    /// This function is primarily used for optimization purposes.
    pub fn will_wake(&self, other: &Waker) -> bool {
        self.waker == other.waker
    }

    /// Creates a new `Waker` from `RawWaker`.
    ///
    /// The method cannot check whether `RawWaker` fulfills the required API
    /// contract to make it usable for `Waker` and is therefore unsafe.
    pub unsafe fn new_unchecked(waker: RawWaker) -> Waker {
        Waker {
            waker: waker,
        }
    }
}

impl Clone for Waker {
    fn clone(&self) -> Self {
        Waker {
            waker: unsafe { (self.waker.vtable.clone)(self.waker.data) },
        }
    }
}

impl Drop for Waker {
    fn drop(&mut self) {
        unsafe { (self.waker.vtable.drop_fn)(self.waker.data) }
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Waker")
            .finish()
    }
}
