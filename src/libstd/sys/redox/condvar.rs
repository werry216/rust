use cell::UnsafeCell;
use intrinsics::{atomic_cxchg, atomic_xadd, atomic_xchg};
use ptr;
use time::Duration;

use super::mutex::{mutex_lock, mutex_unlock, Mutex};

use libc::{futex, FUTEX_WAIT, FUTEX_WAKE, FUTEX_REQUEUE};

pub struct Condvar {
    lock: UnsafeCell<*mut i32>,
    seq: UnsafeCell<i32>
}

impl Condvar {
    pub const fn new() -> Condvar {
        Condvar {
            lock: UnsafeCell::new(ptr::null_mut()),
            seq: UnsafeCell::new(0)
        }
    }

    pub unsafe fn init(&self) {

    }

    pub fn notify_one(&self) {
        unsafe {
            let seq = self.seq.get();

            atomic_xadd(seq, 1);

            let _ = futex(seq, FUTEX_WAKE, 1, 0, ptr::null_mut());
        }
    }

    pub fn notify_all(&self) {
        unsafe {
            let lock = self.lock.get();
            let seq = self.seq.get();

            if *lock == ptr::null_mut() {
                return;
            }

            atomic_xadd(seq, 1);

            let _ = futex(seq, FUTEX_REQUEUE, 1, ::usize::MAX, *lock);
        }
    }

    pub fn wait(&self, mutex: &Mutex) {
        unsafe {
            let lock = self.lock.get();
            let seq = self.seq.get();

            if *lock != mutex.lock.get() {
                if *lock != ptr::null_mut() {
                    panic!("Condvar used with more than one Mutex");
                }

                atomic_cxchg(lock as *mut usize, 0, mutex.lock.get() as usize);
            }

            mutex_unlock(*lock);

            let _ = futex(seq, FUTEX_WAIT, *seq, 0, ptr::null_mut());

            while atomic_xchg(*lock, 2) != 0 {
                let _ = futex(*lock, FUTEX_WAIT, 2, 0, ptr::null_mut());
            }

            mutex_lock(*lock);
        }
    }

    pub fn wait_timeout(&self, _mutex: &Mutex, _dur: Duration) -> bool {
        unimplemented!();
    }

    pub unsafe fn destroy(&self) {

    }
}

unsafe impl Send for Condvar {}

unsafe impl Sync for Condvar {}
