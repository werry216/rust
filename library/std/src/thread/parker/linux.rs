use crate::sync::atomic::AtomicI32;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::time::Duration;

const PARKED: i32 = -1;
const EMPTY: i32 = 0;
const NOTIFIED: i32 = 1;

pub struct Parker {
    state: AtomicI32,
}

impl Parker {
    #[inline]
    pub const fn new() -> Self {
        Parker { state: AtomicI32::new(EMPTY) }
    }

    // Assumes this is only called by the thread that owns the Parker,
    // which means that `self.state != PARKED`.
    pub unsafe fn park(&self) {
        // Change NOTIFIED=>EMPTY or EMPTY=>PARKED, and directly return in the
        // first case.
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }
        loop {
            // Wait for something to happen, assuming it's still set to PARKED.
            futex_wait(&self.state, PARKED, None);
            // Change NOTIFIED=>EMPTY and return in that case.
            if self.state.compare_and_swap(NOTIFIED, EMPTY, Acquire) == NOTIFIED {
                return;
            } else {
                // Spurious wake up. We loop to try again.
            }
        }
    }

    // Assumes this is only called by the thread that owns the Parker,
    // which means that `self.state != PARKED`.
    pub unsafe fn park_timeout(&self, timeout: Duration) {
        // Change NOTIFIED=>EMPTY or EMPTY=>PARKED, and directly return in the
        // first case.
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }
        // Wait for something to happen, assuming it's still set to PARKED.
        futex_wait(&self.state, PARKED, Some(timeout));
        // This is not just a store, because we need to establish a
        // release-acquire ordering with unpark().
        if self.state.swap(EMPTY, Acquire) == NOTIFIED {
            // Woke up because of unpark().
        } else {
            // Timeout or spurious wake up.
            // We return either way, because we can't easily tell if it was the
            // timeout or not.
        }
    }

    #[inline]
    pub fn unpark(&self) {
        // Change PARKED=>NOTIFIED, EMPTY=>NOTIFIED, or NOTIFIED=>NOTIFIED, and
        // wake the thread in the first case.
        //
        // Note that even NOTIFIED=>NOTIFIED results in a write. This is on
        // purpose, to make sure every unpark() has a release-acquire ordering
        // with park().
        if self.state.swap(NOTIFIED, Release) == PARKED {
            futex_wake(&self.state);
        }
    }
}

fn futex_wait(futex: &AtomicI32, expected: i32, timeout: Option<Duration>) {
    let timespec;
    let timespec_ptr = match timeout {
        Some(timeout) => {
            timespec = libc::timespec {
                tv_sec: timeout.as_secs() as _,
                tv_nsec: timeout.subsec_nanos() as _,
            };
            &timespec as *const libc::timespec
        }
        None => crate::ptr::null(),
    };
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            futex as *const AtomicI32,
            libc::FUTEX_WAIT | libc::FUTEX_PRIVATE_FLAG,
            expected,
            timespec_ptr,
        );
    }
}

fn futex_wake(futex: &AtomicI32) {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            futex as *const AtomicI32,
            libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG,
            1,
        );
    }
}
