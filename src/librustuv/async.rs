// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cast;
use std::libc::c_int;
use std::rt::rtio::{Callback, RemoteCallback};
use std::unstable::sync::Exclusive;

use uvll;
use super::{Loop, UvHandle};

// The entire point of async is to call into a loop from other threads so it
// does not need to home.
pub struct AsyncWatcher {
    handle: *uvll::uv_async_t,

    // A flag to tell the callback to exit, set from the dtor. This is
    // almost never contested - only in rare races with the dtor.
    exit_flag: Exclusive<bool>
}

struct Payload {
    callback: ~Callback,
    exit_flag: Exclusive<bool>,
}

impl AsyncWatcher {
    pub fn new(loop_: &mut Loop, cb: ~Callback) -> AsyncWatcher {
        let handle = UvHandle::alloc(None::<AsyncWatcher>, uvll::UV_ASYNC);
        assert_eq!(unsafe {
            uvll::uv_async_init(loop_.native_handle(), handle, async_cb)
        }, 0);
        let flag = Exclusive::new(false);
        let payload = ~Payload { callback: cb, exit_flag: flag.clone() };
        unsafe {
            let payload: *u8 = cast::transmute(payload);
            uvll::set_data_for_uv_handle(handle, payload);
        }
        return AsyncWatcher { handle: handle, exit_flag: flag, };
    }
}

impl UvHandle<uvll::uv_async_t> for AsyncWatcher {
    fn uv_handle(&self) -> *uvll::uv_async_t { self.handle }
    unsafe fn from_uv_handle<'a>(_: &'a *uvll::uv_async_t) -> &'a mut AsyncWatcher {
        fail!("async watchers can't be built from their handles");
    }
}

extern fn async_cb(handle: *uvll::uv_async_t, status: c_int) {
    assert!(status == 0);
    let payload: &mut Payload = unsafe {
        cast::transmute(uvll::get_data_for_uv_handle(handle))
    };

    // The synchronization logic here is subtle. To review,
    // the uv async handle type promises that, after it is
    // triggered the remote callback is definitely called at
    // least once. UvRemoteCallback needs to maintain those
    // semantics while also shutting down cleanly from the
    // dtor. In our case that means that, when the
    // UvRemoteCallback dtor calls `async.send()`, here `f` is
    // always called later.

    // In the dtor both the exit flag is set and the async
    // callback fired under a lock.  Here, before calling `f`,
    // we take the lock and check the flag. Because we are
    // checking the flag before calling `f`, and the flag is
    // set under the same lock as the send, then if the flag
    // is set then we're guaranteed to call `f` after the
    // final send.

    // If the check was done after `f()` then there would be a
    // period between that call and the check where the dtor
    // could be called in the other thread, missing the final
    // callback while still destroying the handle.

    let should_exit = unsafe {
        payload.exit_flag.with_imm(|&should_exit| should_exit)
    };

    payload.callback.call();

    if should_exit {
        unsafe { uvll::uv_close(handle, close_cb) }
    }
}

extern fn close_cb(handle: *uvll::uv_handle_t) {
    // drop the payload
    let _payload: ~Payload = unsafe {
        cast::transmute(uvll::get_data_for_uv_handle(handle))
    };
    // and then free the handle
    unsafe { uvll::free_handle(handle) }
}

impl RemoteCallback for AsyncWatcher {
    fn fire(&mut self) {
        unsafe { uvll::uv_async_send(self.handle) }
    }
}

impl Drop for AsyncWatcher {
    fn drop(&mut self) {
        unsafe {
            do self.exit_flag.with |should_exit| {
                // NB: These two things need to happen atomically. Otherwise
                // the event handler could wake up due to a *previous*
                // signal and see the exit flag, destroying the handle
                // before the final send.
                *should_exit = true;
                uvll::uv_async_send(self.handle)
            }
        }
    }
}

#[cfg(test)]
mod test_remote {
    use std::cell::Cell;
    use std::rt::test::*;
    use std::rt::thread::Thread;
    use std::rt::tube::Tube;
    use std::rt::rtio::EventLoop;
    use std::rt::local::Local;
    use std::rt::sched::Scheduler;

    #[test]
    fn test_uv_remote() {
        do run_in_mt_newsched_task {
            let mut tube = Tube::new();
            let tube_clone = tube.clone();
            let remote_cell = Cell::new_empty();
            do Local::borrow |sched: &mut Scheduler| {
                let tube_clone = tube_clone.clone();
                let tube_clone_cell = Cell::new(tube_clone);
                let remote = do sched.event_loop.remote_callback {
                    // This could be called multiple times
                    if !tube_clone_cell.is_empty() {
                        tube_clone_cell.take().send(1);
                    }
                };
                remote_cell.put_back(remote);
            }
            let thread = do Thread::start {
                remote_cell.take().fire();
            };

            assert!(tube.recv() == 1);
            thread.join();
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use Loop;
    use std::unstable::run_in_bare_thread;
    use std::rt::thread::Thread;
    use std::cell::Cell;

    #[test]
    fn smoke_test() {
        do run_in_bare_thread {
            let mut loop_ = Loop::new();
            let watcher = AsyncWatcher::new(&mut loop_, |w, _| w.close(||()) );
            let watcher_cell = Cell::new(watcher);
            let thread = do Thread::start {
                let mut watcher = watcher_cell.take();
                watcher.send();
            };
            loop_.run();
            loop_.close();
            thread.join();
        }
    }
}
