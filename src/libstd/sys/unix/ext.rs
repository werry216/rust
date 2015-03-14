// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Experimental extensions to `std` for Unix platforms.
//!
//! For now, this module is limited to extracting file descriptors,
//! but its functionality will grow over time.
//!
//! # Example
//!
//! ```rust,ignore
//! #![feature(globs)]
//!
//! use std::old_io::fs::File;
//! use std::os::unix::prelude::*;
//!
//! fn main() {
//!     let f = File::create(&Path::new("foo.txt")).unwrap();
//!     let fd = f.as_raw_fd();
//!
//!     // use fd with native unix bindings
//! }
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

/// Unix-specific extensions to general I/O primitives
#[unstable(feature = "io_ext",
           reason = "may want a slightly different organization or a more \
                     general file descriptor primitive")]
pub mod io {
    #[allow(deprecated)] use old_io;
    use fs;
    use libc;
    use net;
    use sys_common::AsInner;

    /// Raw file descriptors.
    pub type Fd = libc::c_int;

    /// Extract raw file descriptor
    pub trait AsRawFd {
        /// Extract the raw file descriptor, without taking any ownership.
        fn as_raw_fd(&self) -> Fd;
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::fs::File {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    impl AsRawFd for fs::File {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd().raw()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::pipe::PipeStream {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::pipe::UnixStream {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::pipe::UnixListener {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::pipe::UnixAcceptor {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::tcp::TcpStream {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::tcp::TcpListener {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::tcp::TcpAcceptor {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    #[allow(deprecated)]
    impl AsRawFd for old_io::net::udp::UdpSocket {
        fn as_raw_fd(&self) -> Fd {
            self.as_inner().fd()
        }
    }

    impl AsRawFd for net::TcpStream {
        fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
    }
    impl AsRawFd for net::TcpListener {
        fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
    }
    impl AsRawFd for net::UdpSocket {
        fn as_raw_fd(&self) -> Fd { *self.as_inner().socket().as_inner() }
    }
}

////////////////////////////////////////////////////////////////////////////////
// OsString and OsStr
////////////////////////////////////////////////////////////////////////////////

/// Unix-specific extension to the primitives in the `std::ffi` module
#[stable(feature = "rust1", since = "1.0.0")]
pub mod ffi {
    use ffi::{CString, NulError, OsStr, OsString};
    use mem;
    use prelude::v1::*;
    use sys::os_str::Buf;
    use sys_common::{FromInner, IntoInner, AsInner};

    /// Unix-specific extensions to `OsString`.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub trait OsStringExt {
        /// Create an `OsString` from a byte vector.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn from_vec(vec: Vec<u8>) -> Self;

        /// Yield the underlying byte vector of this `OsString`.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn into_vec(self) -> Vec<u8>;
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl OsStringExt for OsString {
        fn from_vec(vec: Vec<u8>) -> OsString {
            FromInner::from_inner(Buf { inner: vec })
        }
        fn into_vec(self) -> Vec<u8> {
            self.into_inner().inner
        }
    }

    /// Unix-specific extensions to `OsStr`.
    #[stable(feature = "rust1", since = "1.0.0")]
    pub trait OsStrExt {
        #[stable(feature = "rust1", since = "1.0.0")]
        fn from_bytes(slice: &[u8]) -> &Self;

        /// Get the underlying byte view of the `OsStr` slice.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn as_bytes(&self) -> &[u8];

        /// Convert the `OsStr` slice into a `CString`.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn to_cstring(&self) -> Result<CString, NulError>;
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl OsStrExt for OsStr {
        fn from_bytes(slice: &[u8]) -> &OsStr {
            unsafe { mem::transmute(slice) }
        }
        fn as_bytes(&self) -> &[u8] {
            &self.as_inner().inner
        }
        fn to_cstring(&self) -> Result<CString, NulError> {
            CString::new(self.as_bytes())
        }
    }
}

/// Unix-specific extensions to primitives in the `std::fs` module.
#[unstable(feature = "fs_ext",
           reason = "may want a more useful mode abstraction")]
pub mod fs {
    use sys_common::{FromInner, AsInner, AsInnerMut};
    use fs::{Permissions, OpenOptions};

    /// Unix-specific extensions to `Permissions`
    pub trait PermissionsExt {
        fn mode(&self) -> i32;
        fn set_mode(&mut self, mode: i32);
    }

    impl PermissionsExt for Permissions {
        fn mode(&self) -> i32 { self.as_inner().mode() }

        fn set_mode(&mut self, mode: i32) {
            *self = FromInner::from_inner(FromInner::from_inner(mode));
        }
    }

    /// Unix-specific extensions to `OpenOptions`
    pub trait OpenOptionsExt {
        /// Set the mode bits that a new file will be created with.
        ///
        /// If a new file is created as part of a `File::open_opts` call then this
        /// specified `mode` will be used as the permission bits for the new file.
        fn mode(&mut self, mode: i32) -> &mut Self;
    }

    impl OpenOptionsExt for OpenOptions {
        fn mode(&mut self, mode: i32) -> &mut OpenOptions {
            self.as_inner_mut().mode(mode); self
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Process and Command
////////////////////////////////////////////////////////////////////////////////

/// Unix-specific extensions to primitives in the `std::process` module.
#[stable(feature = "rust1", since = "1.0.0")]
pub mod process {
    use prelude::v1::*;
    use libc::{uid_t, gid_t};
    use process;
    use sys;
    use sys_common::{AsInnerMut, AsInner};

    /// Unix-specific extensions to the `std::process::Command` builder
    #[stable(feature = "rust1", since = "1.0.0")]
    pub trait CommandExt {
        /// Sets the child process's user id. This translates to a
        /// `setuid` call in the child process. Failure in the `setuid`
        /// call will cause the spawn to fail.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn uid(&mut self, id: uid_t) -> &mut process::Command;

        /// Similar to `uid`, but sets the group id of the child process. This has
        /// the same semantics as the `uid` field.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn gid(&mut self, id: gid_t) -> &mut process::Command;
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl CommandExt for process::Command {
        fn uid(&mut self, id: uid_t) -> &mut process::Command {
            self.as_inner_mut().uid = Some(id);
            self
        }

        fn gid(&mut self, id: gid_t) -> &mut process::Command {
            self.as_inner_mut().gid = Some(id);
            self
        }
    }

    /// Unix-specific extensions to `std::process::ExitStatus`
    #[stable(feature = "rust1", since = "1.0.0")]
    pub trait ExitStatusExt {
        /// If the process was terminated by a signal, returns that signal.
        #[stable(feature = "rust1", since = "1.0.0")]
        fn signal(&self) -> Option<i32>;
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl ExitStatusExt for process::ExitStatus {
        fn signal(&self) -> Option<i32> {
            match *self.as_inner() {
                sys::process2::ExitStatus::Signal(s) => Some(s),
                _ => None
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Prelude
////////////////////////////////////////////////////////////////////////////////

/// A prelude for conveniently writing platform-specific code.
///
/// Includes all extension traits, and some important type definitions.
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    pub use super::io::{Fd, AsRawFd};
    #[doc(no_inline)] #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
    #[doc(no_inline)]
    pub use super::fs::{PermissionsExt, OpenOptionsExt};
    #[doc(no_inline)] #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::process::{CommandExt, ExitStatusExt};
}
