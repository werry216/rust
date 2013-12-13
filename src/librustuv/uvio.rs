// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::c_str::CString;
use std::cast;
use std::comm::SharedChan;
use std::io::IoError;
use std::io::net::ip::SocketAddr;
use std::io::process::ProcessConfig;
use std::io::signal::Signum;
use std::io::{FileMode, FileAccess, Open, Append, Truncate, Read, Write,
              ReadWrite, FileStat};
use std::io;
use std::libc::c_int;
use std::libc::{O_CREAT, O_APPEND, O_TRUNC, O_RDWR, O_RDONLY, O_WRONLY, S_IRUSR,
                S_IWUSR};
use std::libc;
use std::path::Path;
use std::rt::rtio;
use std::rt::rtio::IoFactory;
use ai = std::io::net::addrinfo;

#[cfg(test)] use std::unstable::run_in_bare_thread;

use super::{uv_error_to_io_error, Loop};

use addrinfo::GetAddrInfoRequest;
use async::AsyncWatcher;
use file::{FsRequest, FileWatcher};
use queue::QueuePool;
use homing::HomeHandle;
use idle::IdleWatcher;
use net::{TcpWatcher, TcpListener, UdpWatcher};
use pipe::{PipeWatcher, PipeListener};
use process::Process;
use signal::SignalWatcher;
use timer::TimerWatcher;
use tty::TtyWatcher;
use uvll;

// Obviously an Event Loop is always home.
pub struct UvEventLoop {
    priv uvio: UvIoFactory
}

impl UvEventLoop {
    pub fn new() -> UvEventLoop {
        let mut loop_ = Loop::new();
        let handle_pool = QueuePool::new(&mut loop_);
        UvEventLoop {
            uvio: UvIoFactory {
                loop_: loop_,
                handle_pool: handle_pool,
            }
        }
    }
}

impl Drop for UvEventLoop {
    fn drop(&mut self) {
        self.uvio.loop_.close();
    }
}

impl rtio::EventLoop for UvEventLoop {
    fn run(&mut self) {
        self.uvio.loop_.run();
    }

    fn callback(&mut self, f: proc()) {
        IdleWatcher::onetime(&mut self.uvio.loop_, f);
    }

    fn pausible_idle_callback(&mut self, cb: ~rtio::Callback)
        -> ~rtio::PausibleIdleCallback
    {
        IdleWatcher::new(&mut self.uvio.loop_, cb) as ~rtio::PausibleIdleCallback
    }

    fn remote_callback(&mut self, f: ~rtio::Callback) -> ~rtio::RemoteCallback {
        ~AsyncWatcher::new(&mut self.uvio.loop_, f) as ~rtio::RemoteCallback
    }

    fn io<'a>(&'a mut self) -> Option<&'a mut rtio::IoFactory> {
        let factory = &mut self.uvio as &mut rtio::IoFactory;
        Some(factory)
    }
}

#[cfg(not(test))]
#[lang = "event_loop_factory"]
pub extern "C" fn new_loop() -> ~rtio::EventLoop {
    ~UvEventLoop::new() as ~rtio::EventLoop
}

#[test]
fn test_callback_run_once() {
    use std::rt::rtio::EventLoop;
    do run_in_bare_thread {
        let mut event_loop = UvEventLoop::new();
        let mut count = 0;
        let count_ptr: *mut int = &mut count;
        do event_loop.callback {
            unsafe { *count_ptr += 1 }
        }
        event_loop.run();
        assert_eq!(count, 1);
    }
}

pub struct UvIoFactory {
    loop_: Loop,
    priv handle_pool: ~QueuePool,
}

impl UvIoFactory {
    pub fn uv_loop<'a>(&mut self) -> *uvll::uv_loop_t { self.loop_.handle }

    pub fn make_handle(&mut self) -> HomeHandle {
        HomeHandle::new(self.id(), &mut *self.handle_pool)
    }
}

impl IoFactory for UvIoFactory {
    fn id(&self) -> uint { unsafe { cast::transmute(self) } }

    // Connect to an address and return a new stream
    // NB: This blocks the task waiting on the connection.
    // It would probably be better to return a future
    fn tcp_connect(&mut self, addr: SocketAddr)
        -> Result<~rtio::RtioTcpStream, IoError>
    {
        match TcpWatcher::connect(self, addr) {
            Ok(t) => Ok(~t as ~rtio::RtioTcpStream),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn tcp_bind(&mut self, addr: SocketAddr) -> Result<~rtio::RtioTcpListener, IoError> {
        match TcpListener::bind(self, addr) {
            Ok(t) => Ok(t as ~rtio::RtioTcpListener),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn udp_bind(&mut self, addr: SocketAddr) -> Result<~rtio::RtioUdpSocket, IoError> {
        match UdpWatcher::bind(self, addr) {
            Ok(u) => Ok(~u as ~rtio::RtioUdpSocket),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn timer_init(&mut self) -> Result<~rtio::RtioTimer, IoError> {
        Ok(TimerWatcher::new(self) as ~rtio::RtioTimer)
    }

    fn get_host_addresses(&mut self, host: Option<&str>, servname: Option<&str>,
                          hint: Option<ai::Hint>) -> Result<~[ai::Info], IoError> {
        let r = GetAddrInfoRequest::run(&self.loop_, host, servname, hint);
        r.map_err(uv_error_to_io_error)
    }

    fn fs_from_raw_fd(&mut self, fd: c_int,
                      close: rtio::CloseBehavior) -> ~rtio::RtioFileStream {
        ~FileWatcher::new(self, fd, close) as ~rtio::RtioFileStream
    }

    fn fs_open(&mut self, path: &CString, fm: FileMode, fa: FileAccess)
        -> Result<~rtio::RtioFileStream, IoError> {
        let flags = match fm {
            io::Open => 0,
            io::Append => libc::O_APPEND,
            io::Truncate => libc::O_TRUNC,
        };
        // Opening with a write permission must silently create the file.
        let (flags, mode) = match fa {
            io::Read => (flags | libc::O_RDONLY, 0),
            io::Write => (flags | libc::O_WRONLY | libc::O_CREAT,
                          libc::S_IRUSR | libc::S_IWUSR),
            io::ReadWrite => (flags | libc::O_RDWR | libc::O_CREAT,
                              libc::S_IRUSR | libc::S_IWUSR),
        };

        match FsRequest::open(self, path, flags as int, mode as int) {
            Ok(fs) => Ok(~fs as ~rtio::RtioFileStream),
            Err(e) => Err(uv_error_to_io_error(e))
        }
    }

    fn fs_unlink(&mut self, path: &CString) -> Result<(), IoError> {
        let r = FsRequest::unlink(&self.loop_, path);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_lstat(&mut self, path: &CString) -> Result<FileStat, IoError> {
        let r = FsRequest::lstat(&self.loop_, path);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_stat(&mut self, path: &CString) -> Result<FileStat, IoError> {
        let r = FsRequest::stat(&self.loop_, path);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_mkdir(&mut self, path: &CString,
                perm: io::FilePermission) -> Result<(), IoError> {
        let r = FsRequest::mkdir(&self.loop_, path, perm as c_int);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_rmdir(&mut self, path: &CString) -> Result<(), IoError> {
        let r = FsRequest::rmdir(&self.loop_, path);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_rename(&mut self, path: &CString, to: &CString) -> Result<(), IoError> {
        let r = FsRequest::rename(&self.loop_, path, to);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_chmod(&mut self, path: &CString,
                perm: io::FilePermission) -> Result<(), IoError> {
        let r = FsRequest::chmod(&self.loop_, path, perm as c_int);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_readdir(&mut self, path: &CString, flags: c_int)
        -> Result<~[Path], IoError>
    {
        let r = FsRequest::readdir(&self.loop_, path, flags);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_link(&mut self, src: &CString, dst: &CString) -> Result<(), IoError> {
        let r = FsRequest::link(&self.loop_, src, dst);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_symlink(&mut self, src: &CString, dst: &CString) -> Result<(), IoError> {
        let r = FsRequest::symlink(&self.loop_, src, dst);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_chown(&mut self, path: &CString, uid: int, gid: int) -> Result<(), IoError> {
        let r = FsRequest::chown(&self.loop_, path, uid, gid);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_readlink(&mut self, path: &CString) -> Result<Path, IoError> {
        let r = FsRequest::readlink(&self.loop_, path);
        r.map_err(uv_error_to_io_error)
    }
    fn fs_utime(&mut self, path: &CString, atime: u64, mtime: u64)
        -> Result<(), IoError>
    {
        let r = FsRequest::utime(&self.loop_, path, atime, mtime);
        r.map_err(uv_error_to_io_error)
    }

    fn spawn(&mut self, config: ProcessConfig)
            -> Result<(~rtio::RtioProcess, ~[Option<~rtio::RtioPipe>]), IoError>
    {
        match Process::spawn(self, config) {
            Ok((p, io)) => {
                Ok((p as ~rtio::RtioProcess,
                    io.move_iter().map(|i| i.map(|p| ~p as ~rtio::RtioPipe)).collect()))
            }
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn unix_bind(&mut self, path: &CString) -> Result<~rtio::RtioUnixListener, IoError>
    {
        match PipeListener::bind(self, path) {
            Ok(p) => Ok(p as ~rtio::RtioUnixListener),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn unix_connect(&mut self, path: &CString) -> Result<~rtio::RtioPipe, IoError> {
        match PipeWatcher::connect(self, path) {
            Ok(p) => Ok(~p as ~rtio::RtioPipe),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }

    fn tty_open(&mut self, fd: c_int, readable: bool)
            -> Result<~rtio::RtioTTY, IoError> {
        match TtyWatcher::new(self, fd, readable) {
            Ok(tty) => Ok(~tty as ~rtio::RtioTTY),
            Err(e) => Err(uv_error_to_io_error(e))
        }
    }

    fn pipe_open(&mut self, fd: c_int) -> Result<~rtio::RtioPipe, IoError> {
        match PipeWatcher::open(self, fd) {
            Ok(s) => Ok(~s as ~rtio::RtioPipe),
            Err(e) => Err(uv_error_to_io_error(e))
        }
    }

    fn signal(&mut self, signum: Signum, channel: SharedChan<Signum>)
        -> Result<~rtio::RtioSignal, IoError> {
        match SignalWatcher::new(self, signum, channel) {
            Ok(s) => Ok(s as ~rtio::RtioSignal),
            Err(e) => Err(uv_error_to_io_error(e)),
        }
    }
}
