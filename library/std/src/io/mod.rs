//! Traits, helpers, and type definitions for core I/O functionality.
//!
//! The `std::io` module contains a number of common things you'll need
//! when doing input and output. The most core part of this module is
//! the [`Read`] and [`Write`] traits, which provide the
//! most general interface for reading and writing input and output.
//!
//! # Read and Write
//!
//! Because they are traits, [`Read`] and [`Write`] are implemented by a number
//! of other types, and you can implement them for your types too. As such,
//! you'll see a few different types of I/O throughout the documentation in
//! this module: [`File`]s, [`TcpStream`]s, and sometimes even [`Vec<T>`]s. For
//! example, [`Read`] adds a [`read`][`Read::read`] method, which we can use on
//! [`File`]s:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // read up to 10 bytes
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`Read`] and [`Write`] are so important, implementors of the two traits have a
//! nickname: readers and writers. So you'll sometimes see 'a reader' instead
//! of 'a type that implements the [`Read`] trait'. Much easier!
//!
//! ## Seek and BufRead
//!
//! Beyond that, there are two important traits that are provided: [`Seek`]
//! and [`BufRead`]. Both of these build on top of a reader to control
//! how the reading happens. [`Seek`] lets you control where the next byte is
//! coming from:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::SeekFrom;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // skip to the last 10 bytes of the file
//!     f.seek(SeekFrom::End(-10))?;
//!
//!     // read up to 10 bytes
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`BufRead`] uses an internal buffer to provide a number of other ways to read, but
//! to show it off, we'll need to talk about buffers in general. Keep reading!
//!
//! ## BufReader and BufWriter
//!
//! Byte-based interfaces are unwieldy and can be inefficient, as we'd need to be
//! making near-constant calls to the operating system. To help with this,
//! `std::io` comes with two structs, [`BufReader`] and [`BufWriter`], which wrap
//! readers and writers. The wrapper uses a buffer, reducing the number of
//! calls and providing nicer methods for accessing exactly what you want.
//!
//! For example, [`BufReader`] works with the [`BufRead`] trait to add extra
//! methods to any reader:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::open("foo.txt")?;
//!     let mut reader = BufReader::new(f);
//!     let mut buffer = String::new();
//!
//!     // read a line into buffer
//!     reader.read_line(&mut buffer)?;
//!
//!     println!("{}", buffer);
//!     Ok(())
//! }
//! ```
//!
//! [`BufWriter`] doesn't add any new ways of writing; it just buffers every call
//! to [`write`][`Write::write`]:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufWriter;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::create("foo.txt")?;
//!     {
//!         let mut writer = BufWriter::new(f);
//!
//!         // write a byte to the buffer
//!         writer.write(&[42])?;
//!
//!     } // the buffer is flushed once writer goes out of scope
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Standard input and output
//!
//! A very common source of input is standard input:
//!
//! ```no_run
//! use std::io;
//!
//! fn main() -> io::Result<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!     Ok(())
//! }
//! ```
//!
//! Note that you cannot use the [`?` operator] in functions that do not return
//! a [`Result<T, E>`][`Result`]. Instead, you can call [`.unwrap()`]
//! or `match` on the return value to catch any possible errors:
//!
//! ```no_run
//! use std::io;
//!
//! let mut input = String::new();
//!
//! io::stdin().read_line(&mut input).unwrap();
//! ```
//!
//! And a very common source of output is standard output:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//!
//! fn main() -> io::Result<()> {
//!     io::stdout().write(&[42])?;
//!     Ok(())
//! }
//! ```
//!
//! Of course, using [`io::stdout`] directly is less common than something like
//! [`println!`].
//!
//! ## Iterator types
//!
//! A large number of the structures provided by `std::io` are for various
//! ways of iterating over I/O. For example, [`Lines`] is used to split over
//! lines:
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::open("foo.txt")?;
//!     let reader = BufReader::new(f);
//!
//!     for line in reader.lines() {
//!         println!("{}", line?);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Functions
//!
//! There are a number of [functions][functions-list] that offer access to various
//! features. For example, we can use three of these functions to copy everything
//! from standard input to standard output:
//!
//! ```no_run
//! use std::io;
//!
//! fn main() -> io::Result<()> {
//!     io::copy(&mut io::stdin(), &mut io::stdout())?;
//!     Ok(())
//! }
//! ```
//!
//! [functions-list]: #functions-1
//!
//! ## io::Result
//!
//! Last, but certainly not least, is [`io::Result`]. This type is used
//! as the return type of many `std::io` functions that can cause an error, and
//! can be returned from your own functions as well. Many of the examples in this
//! module use the [`?` operator]:
//!
//! ```
//! use std::io;
//!
//! fn read_input() -> io::Result<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!
//!     Ok(())
//! }
//! ```
//!
//! The return type of `read_input()`, [`io::Result<()>`][`io::Result`], is a very
//! common type for functions which don't have a 'real' return value, but do want to
//! return errors if they happen. In this case, the only purpose of this function is
//! to read the line and print it, so we use `()`.
//!
//! ## Platform-specific behavior
//!
//! Many I/O functions throughout the standard library are documented to indicate
//! what various library or syscalls they are delegated to. This is done to help
//! applications both understand what's happening under the hood as well as investigate
//! any possibly unclear semantics. Note, however, that this is informative, not a binding
//! contract. The implementation of many of these functions are subject to change over
//! time and may call fewer or more syscalls/library functions.
//!
//! [`File`]: crate::fs::File
//! [`TcpStream`]: crate::net::TcpStream
//! [`io::stdout`]: stdout
//! [`io::Result`]: self::Result
//! [`?` operator]: ../../book/appendix-02-operators.html
//! [`Result`]: crate::result::Result
//! [`.unwrap()`]: crate::result::Result::unwrap

#![stable(feature = "rust1", since = "1.0.0")]

#[cfg(test)]
mod tests;

use crate::cmp;
use crate::fmt;
use crate::memchr;
use crate::ops::{Deref, DerefMut};
use crate::ptr;
use crate::slice;
use crate::str;
use crate::sys;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::buffered::IntoInnerError;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::buffered::{BufReader, BufWriter, LineWriter};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::copy::copy;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::cursor::Cursor;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::error::{Error, ErrorKind, Result};
#[unstable(feature = "internal_output_capture", issue = "none")]
#[doc(no_inline, hidden)]
pub use self::stdio::set_output_capture;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::stdio::{stderr, stdin, stdout, Stderr, Stdin, Stdout};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::stdio::{StderrLock, StdinLock, StdoutLock};
#[unstable(feature = "print_internals", issue = "none")]
pub use self::stdio::{_eprint, _print};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::util::{empty, repeat, sink, Empty, Repeat, Sink};

mod buffered;
pub(crate) mod copy;
mod cursor;
mod error;
mod impls;
pub mod prelude;
mod stdio;
mod util;

const DEFAULT_BUF_SIZE: usize = crate::sys_common::io::DEFAULT_BUF_SIZE;

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

// A few methods below (read_to_string, read_line) will append data into a
// `String` buffer, but we need to be pretty careful when doing this. The
// implementation will just call `.as_mut_vec()` and then delegate to a
// byte-oriented reading method, but we must ensure that when returning we never
// leave `buf` in a state such that it contains invalid UTF-8 in its bounds.
//
// To this end, we use an RAII guard (to protect against panics) which updates
// the length of the string when it is dropped. This guard initially truncates
// the string to the prior length and only after we've validated that the
// new contents are valid UTF-8 do we allow it to set a longer length.
//
// The unsafety in this function is twofold:
//
// 1. We're looking at the raw bytes of `buf`, so we take on the burden of UTF-8
//    checks.
// 2. We're passing a raw buffer to the function `f`, and it is expected that
//    the function only *appends* bytes to the buffer. We'll get undefined
//    behavior if existing bytes are overwritten to have non-UTF-8 data.
fn append_to_string<F>(buf: &mut String, f: F) -> Result<usize>
where
    F: FnOnce(&mut Vec<u8>) -> Result<usize>,
{
    unsafe {
        let mut g = Guard { len: buf.len(), buf: buf.as_mut_vec() };
        let ret = f(g.buf);
        if str::from_utf8(&g.buf[g.len..]).is_err() {
            ret.and_then(|_| {
                Err(Error::new(ErrorKind::InvalidData, "stream did not contain valid UTF-8"))
            })
        } else {
            g.len = g.buf.len();
            ret
        }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than a default reservation size of 32 if the
// reader has a very small amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
fn read_to_end<R: Read + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize> {
    read_to_end_with_reservation(r, buf, |_| 32)
}

fn read_to_end_with_reservation<R, F>(
    r: &mut R,
    buf: &mut Vec<u8>,
    mut reservation_size: F,
) -> Result<usize>
where
    R: Read + ?Sized,
    F: FnMut(&R) -> usize,
{
    let start_len = buf.len();
    let mut g = Guard { len: buf.len(), buf };
    loop {
        if g.len == g.buf.len() {
            unsafe {
                // FIXME(danielhenrymantilla): #42788
                //
                //   - This creates a (mut) reference to a slice of
                //     _uninitialized_ integers, which is **undefined behavior**
                //
                //   - Only the standard library gets to soundly "ignore" this,
                //     based on its privileged knowledge of unstable rustc
                //     internals;
                g.buf.reserve(reservation_size(r));
                let capacity = g.buf.capacity();
                g.buf.set_len(capacity);
                r.initializer().initialize(&mut g.buf[g.len..]);
            }
        }

        let buf = &mut g.buf[g.len..];
        match r.read(buf) {
            Ok(0) => return Ok(g.len - start_len),
            Ok(n) => {
                // We can't allow bogus values from read. If it is too large, the returned vec could have its length
                // set past its capacity, or if it overflows the vec could be shortened which could create an invalid
                // string if this is called via read_to_string.
                assert!(n <= buf.len());
                g.len += n;
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

pub(crate) fn default_read_vectored<F>(read: F, bufs: &mut [IoSliceMut<'_>]) -> Result<usize>
where
    F: FnOnce(&mut [u8]) -> Result<usize>,
{
    let buf = bufs.iter_mut().find(|b| !b.is_empty()).map_or(&mut [][..], |b| &mut **b);
    read(buf)
}

pub(crate) fn default_write_vectored<F>(write: F, bufs: &[IoSlice<'_>]) -> Result<usize>
where
    F: FnOnce(&[u8]) -> Result<usize>,
{
    let buf = bufs.iter().find(|b| !b.is_empty()).map_or(&[][..], |b| &**b);
    write(buf)
}

pub(crate) fn default_read_exact<R: Read + ?Sized>(this: &mut R, mut buf: &mut [u8]) -> Result<()> {
    while !buf.is_empty() {
        match this.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                let tmp = buf;
                buf = &mut tmp[n..];
            }
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    if !buf.is_empty() {
        Err(Error::new(ErrorKind::UnexpectedEof, "failed to fill whole buffer"))
    } else {
        Ok(())
    }
}

/// The `Read` trait allows for reading bytes from a source.
///
/// Implementors of the `Read` trait are called 'readers'.
///
/// Readers are defined by one required method, [`read()`]. Each call to [`read()`]
/// will attempt to pull bytes from this source into a provided buffer. A
/// number of other methods are implemented in terms of [`read()`], giving
/// implementors a number of ways to read bytes while only needing to implement
/// a single method.
///
/// Readers are intended to be composable with one another. Many implementors
/// throughout [`std::io`] take and provide types which implement the `Read`
/// trait.
///
/// Please note that each call to [`read()`] may involve a system call, and
/// therefore, using something that implements [`BufRead`], such as
/// [`BufReader`], will be more efficient.
///
/// # Examples
///
/// [`File`]s implement `Read`:
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> io::Result<()> {
///     let mut f = File::open("foo.txt")?;
///     let mut buffer = [0; 10];
///
///     // read up to 10 bytes
///     f.read(&mut buffer)?;
///
///     let mut buffer = Vec::new();
///     // read the whole file
///     f.read_to_end(&mut buffer)?;
///
///     // read into a String, so that you don't need to do the conversion.
///     let mut buffer = String::new();
///     f.read_to_string(&mut buffer)?;
///
///     // and more! See the other methods for more details.
///     Ok(())
/// }
/// ```
///
/// Read from [`&str`] because [`&[u8]`][slice] implements `Read`:
///
/// ```no_run
/// # use std::io;
/// use std::io::prelude::*;
///
/// fn main() -> io::Result<()> {
///     let mut b = "This string will be read".as_bytes();
///     let mut buffer = [0; 10];
///
///     // read up to 10 bytes
///     b.read(&mut buffer)?;
///
///     // etc... it works exactly as a File does!
///     Ok(())
/// }
/// ```
///
/// [`read()`]: Read::read
/// [`&str`]: prim@str
/// [`std::io`]: self
/// [`File`]: crate::fs::File
/// [slice]: ../../std/primitive.slice.html
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(spotlight)]
pub trait Read {
    /// Pull some bytes from this source into the specified buffer, returning
    /// how many bytes were read.
    ///
    /// This function does not provide any guarantees about whether it blocks
    /// waiting for data, but if an object needs to block for a read and cannot,
    /// it will typically signal this via an [`Err`] return value.
    ///
    /// If the return value of this method is [`Ok(n)`], then it must be
    /// guaranteed that `0 <= n <= buf.len()`. A nonzero `n` value indicates
    /// that the buffer `buf` has been filled in with `n` bytes of data from this
    /// source. If `n` is `0`, then it can indicate one of two scenarios:
    ///
    /// 1. This reader has reached its "end of file" and will likely no longer
    ///    be able to produce bytes. Note that this does not mean that the
    ///    reader will *always* no longer be able to produce bytes.
    /// 2. The buffer specified was 0 bytes in length.
    ///
    /// It is not an error if the returned value `n` is smaller than the buffer size,
    /// even when the reader is not at the end of the stream yet.
    /// This may happen for example because fewer bytes are actually available right now
    /// (e. g. being close to end-of-file) or because read() was interrupted by a signal.
    ///
    /// No guarantees are provided about the contents of `buf` when this
    /// function is called, implementations cannot rely on any property of the
    /// contents of `buf` being true. It is recommended that *implementations*
    /// only write data to `buf` instead of reading its contents.
    ///
    /// Correspondingly, however, *callers* of this method may not assume any guarantees
    /// about how the implementation uses `buf`. The trait is safe to implement,
    /// so it is possible that the code that's supposed to write to the buffer might also read
    /// from it. It is your responsibility to make sure that `buf` is initialized
    /// before calling `read`. Calling `read` with an uninitialized `buf` (of the kind one
    /// obtains via [`MaybeUninit<T>`]) is not safe, and can lead to undefined behavior.
    ///
    /// [`MaybeUninit<T>`]: crate::mem::MaybeUninit
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. If an error is returned then it must be
    /// guaranteed that no bytes were read.
    ///
    /// An error of the [`ErrorKind::Interrupted`] kind is non-fatal and the read
    /// operation should be retried if there is nothing else to do.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`Ok(n)`]: Ok
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // read up to 10 bytes
    ///     let n = f.read(&mut buffer[..])?;
    ///
    ///     println!("The bytes: {:?}", &buffer[..n]);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Like `read`, except that it reads into a slice of buffers.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer
    /// written to possibly being only partially filled. This method must
    /// behave equivalently to a single call to `read` with concatenated
    /// buffers.
    ///
    /// The default implementation calls `read` with either the first nonempty
    /// buffer provided, or an empty one if none exists.
    #[stable(feature = "iovec", since = "1.36.0")]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        default_read_vectored(|b| self.read(b), bufs)
    }

    /// Determines if this `Read`er has an efficient `read_vectored`
    /// implementation.
    ///
    /// If a `Read`er does not override the default `read_vectored`
    /// implementation, code using it may want to avoid the method all together
    /// and coalesce writes into a single buffer for higher performance.
    ///
    /// The default implementation returns `false`.
    #[unstable(feature = "can_vector", issue = "69941")]
    fn is_read_vectored(&self) -> bool {
        false
    }

    /// Determines if this `Read`er can work with buffers of uninitialized
    /// memory.
    ///
    /// The default implementation returns an initializer which will zero
    /// buffers.
    ///
    /// If a `Read`er guarantees that it can work properly with uninitialized
    /// memory, it should call [`Initializer::nop()`]. See the documentation for
    /// [`Initializer`] for details.
    ///
    /// The behavior of this method must be independent of the state of the
    /// `Read`er - the method only takes `&self` so that it can be used through
    /// trait objects.
    ///
    /// # Safety
    ///
    /// This method is unsafe because a `Read`er could otherwise return a
    /// non-zeroing `Initializer` from another `Read` type without an `unsafe`
    /// block.
    #[unstable(feature = "read_initializer", issue = "42788")]
    #[inline]
    unsafe fn initializer(&self) -> Initializer {
        Initializer::zeroing()
    }

    /// Read all bytes until EOF in this source, placing them into `buf`.
    ///
    /// All bytes read from this source will be appended to the specified buffer
    /// `buf`. This function will continuously call [`read()`] to append more data to
    /// `buf` until [`read()`] returns either [`Ok(0)`] or an error of
    /// non-[`ErrorKind::Interrupted`] kind.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Errors
    ///
    /// If this function encounters an error of the kind
    /// [`ErrorKind::Interrupted`] then the error is ignored and the operation
    /// will continue.
    ///
    /// If any other read error is encountered then this function immediately
    /// returns. Any bytes which have already been read will be appended to
    /// `buf`.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`read()`]: Read::read
    /// [`Ok(0)`]: Ok
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///
    ///     // read the whole file
    ///     f.read_to_end(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// (See also the [`std::fs::read`] convenience function for reading from a
    /// file.)
    ///
    /// [`std::fs::read`]: crate::fs::read
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        read_to_end(self, buf)
    }

    /// Read all bytes until EOF in this source, appending them to `buf`.
    ///
    /// If successful, this function returns the number of bytes which were read
    /// and appended to `buf`.
    ///
    /// # Errors
    ///
    /// If the data in this stream is *not* valid UTF-8 then an error is
    /// returned and `buf` is unchanged.
    ///
    /// See [`read_to_end`] for other error semantics.
    ///
    /// [`read_to_end`]: Read::read_to_end
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = String::new();
    ///
    ///     f.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// (See also the [`std::fs::read_to_string`] convenience function for
    /// reading from a file.)
    ///
    /// [`std::fs::read_to_string`]: crate::fs::read_to_string
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        // Note that we do *not* call `.read_to_end()` here. We are passing
        // `&mut Vec<u8>` (the raw contents of `buf`) into the `read_to_end`
        // method to fill it up. An arbitrary implementation could overwrite the
        // entire contents of the vector, not just append to it (which is what
        // we are expecting).
        //
        // To prevent extraneously checking the UTF-8-ness of the entire buffer
        // we pass it to our hardcoded `read_to_end` implementation which we
        // know is guaranteed to only read data into the end of the buffer.
        append_to_string(buf, |b| read_to_end(self, b))
    }

    /// Read the exact number of bytes required to fill `buf`.
    ///
    /// This function reads as many bytes as necessary to completely fill the
    /// specified buffer `buf`.
    ///
    /// No guarantees are provided about the contents of `buf` when this
    /// function is called, implementations cannot rely on any property of the
    /// contents of `buf` being true. It is recommended that implementations
    /// only write data to `buf` instead of reading its contents. The
    /// documentation on [`read`] has a more detailed explanation on this
    /// subject.
    ///
    /// # Errors
    ///
    /// If this function encounters an error of the kind
    /// [`ErrorKind::Interrupted`] then the error is ignored and the operation
    /// will continue.
    ///
    /// If this function encounters an "end of file" before completely filling
    /// the buffer, it returns an error of the kind [`ErrorKind::UnexpectedEof`].
    /// The contents of `buf` are unspecified in this case.
    ///
    /// If any other read error is encountered then this function immediately
    /// returns. The contents of `buf` are unspecified in this case.
    ///
    /// If this function returns an error, it is unspecified how many bytes it
    /// has read, but it will never read more than would be necessary to
    /// completely fill the buffer.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`read`]: Read::read
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // read exactly 10 bytes
    ///     f.read_exact(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "read_exact", since = "1.6.0")]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        default_read_exact(self, buf)
    }

    /// Creates a "by reference" adaptor for this instance of `Read`.
    ///
    /// The returned adaptor also implements `Read` and will simply borrow this
    /// current reader.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::Read;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///     let mut other_buffer = Vec::new();
    ///
    ///     {
    ///         let reference = f.by_ref();
    ///
    ///         // read at most 5 bytes
    ///         reference.take(5).read_to_end(&mut buffer)?;
    ///
    ///     } // drop our &mut reference so we can use f again
    ///
    ///     // original file still usable, read the rest
    ///     f.read_to_end(&mut other_buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    /// Transforms this `Read` instance to an [`Iterator`] over its bytes.
    ///
    /// The returned type implements [`Iterator`] where the `Item` is
    /// [`Result`]`<`[`u8`]`, `[`io::Error`]`>`.
    /// The yielded item is [`Ok`] if a byte was successfully read and [`Err`]
    /// otherwise. EOF is mapped to returning [`None`] from this iterator.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`File`]: crate::fs::File
    /// [`Result`]: crate::result::Result
    /// [`io::Error`]: self::Error
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///
    ///     for byte in f.bytes() {
    ///         println!("{}", byte.unwrap());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bytes(self) -> Bytes<Self>
    where
        Self: Sized,
    {
        Bytes { inner: self }
    }

    /// Creates an adaptor which will chain this stream with another.
    ///
    /// The returned `Read` instance will first read all bytes from this object
    /// until EOF is encountered. Afterwards the output is equivalent to the
    /// output of `next`.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f1 = File::open("foo.txt")?;
    ///     let mut f2 = File::open("bar.txt")?;
    ///
    ///     let mut handle = f1.chain(f2);
    ///     let mut buffer = String::new();
    ///
    ///     // read the value into a String. We could use any Read method here,
    ///     // this is just one example.
    ///     handle.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn chain<R: Read>(self, next: R) -> Chain<Self, R>
    where
        Self: Sized,
    {
        Chain { first: self, second: next, done_first: false }
    }

    /// Creates an adaptor which will read at most `limit` bytes from it.
    ///
    /// This function returns a new instance of `Read` which will read at most
    /// `limit` bytes, after which it will always return EOF ([`Ok(0)`]). Any
    /// read errors will not count towards the number of bytes read and future
    /// calls to [`read()`] may succeed.
    ///
    /// # Examples
    ///
    /// [`File`]s implement `Read`:
    ///
    /// [`File`]: crate::fs::File
    /// [`Ok(0)`]: Ok
    /// [`read()`]: Read::read
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 5];
    ///
    ///     // read at most five bytes
    ///     let mut handle = f.take(5);
    ///
    ///     handle.read(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        Take { inner: self, limit }
    }
}

/// Read all bytes from a [reader][Read] into a new [`String`].
///
/// This is a convenience function for [`Read::read_to_string`]. Using this
/// function avoids having to create a variable first and provides more type
/// safety since you can only get the buffer out if there were no errors. (If you
/// use [`Read::read_to_string`] you have to remember to check whether the read
/// succeeded because otherwise your buffer will be empty or only partially full.)
///
/// # Performance
///
/// The downside of this function's increased ease of use and type safety is
/// that it gives you less control over performance. For example, you can't
/// pre-allocate memory like you can using [`String::with_capacity`] and
/// [`Read::read_to_string`]. Also, you can't re-use the buffer if an error
/// occurs while reading.
///
/// In many cases, this function's performance will be adequate and the ease of use
/// and type safety tradeoffs will be worth it. However, there are cases where you
/// need more control over performance, and in those cases you should definitely use
/// [`Read::read_to_string`] directly.
///
/// # Errors
///
/// This function forces you to handle errors because the output (the `String`)
/// is wrapped in a [`Result`]. See [`Read::read_to_string`] for the errors
/// that can occur. If any error occurs, you will get an [`Err`], so you
/// don't have to worry about your buffer being empty or partially full.
///
/// # Examples
///
/// ```no_run
/// #![feature(io_read_to_string)]
///
/// # use std::io;
/// fn main() -> io::Result<()> {
///     let stdin = io::read_to_string(&mut io::stdin())?;
///     println!("Stdin was:");
///     println!("{}", stdin);
///     Ok(())
/// }
/// ```
#[unstable(feature = "io_read_to_string", issue = "80218")]
pub fn read_to_string<R: Read>(reader: &mut R) -> Result<String> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    Ok(buf)
}

/// A buffer type used with `Read::read_vectored`.
///
/// It is semantically a wrapper around an `&mut [u8]`, but is guaranteed to be
/// ABI compatible with the `iovec` type on Unix platforms and `WSABUF` on
/// Windows.
#[stable(feature = "iovec", since = "1.36.0")]
#[repr(transparent)]
pub struct IoSliceMut<'a>(sys::io::IoSliceMut<'a>);

#[stable(feature = "iovec-send-sync", since = "1.44.0")]
unsafe impl<'a> Send for IoSliceMut<'a> {}

#[stable(feature = "iovec-send-sync", since = "1.44.0")]
unsafe impl<'a> Sync for IoSliceMut<'a> {}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> fmt::Debug for IoSliceMut<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), fmt)
    }
}

impl<'a> IoSliceMut<'a> {
    /// Creates a new `IoSliceMut` wrapping a byte slice.
    ///
    /// # Panics
    ///
    /// Panics on Windows if the slice is larger than 4GB.
    #[stable(feature = "iovec", since = "1.36.0")]
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> IoSliceMut<'a> {
        IoSliceMut(sys::io::IoSliceMut::new(buf))
    }

    /// Advance the internal cursor of the slice.
    ///
    /// # Notes
    ///
    /// Elements in the slice may be modified if the cursor is not advanced to
    /// the end of the slice. For example if we have a slice of buffers with 2
    /// `IoSliceMut`s, both of length 8, and we advance the cursor by 10 bytes
    /// the first `IoSliceMut` will be untouched however the second will be
    /// modified to remove the first 2 bytes (10 - 8).
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(io_slice_advance)]
    ///
    /// use std::io::IoSliceMut;
    /// use std::ops::Deref;
    ///
    /// let mut buf1 = [1; 8];
    /// let mut buf2 = [2; 16];
    /// let mut buf3 = [3; 8];
    /// let mut bufs = &mut [
    ///     IoSliceMut::new(&mut buf1),
    ///     IoSliceMut::new(&mut buf2),
    ///     IoSliceMut::new(&mut buf3),
    /// ][..];
    ///
    /// // Mark 10 bytes as read.
    /// bufs = IoSliceMut::advance(bufs, 10);
    /// assert_eq!(bufs[0].deref(), [2; 14].as_ref());
    /// assert_eq!(bufs[1].deref(), [3; 8].as_ref());
    /// ```
    #[unstable(feature = "io_slice_advance", issue = "62726")]
    #[inline]
    pub fn advance<'b>(bufs: &'b mut [IoSliceMut<'a>], n: usize) -> &'b mut [IoSliceMut<'a>] {
        // Number of buffers to remove.
        let mut remove = 0;
        // Total length of all the to be removed buffers.
        let mut accumulated_len = 0;
        for buf in bufs.iter() {
            if accumulated_len + buf.len() > n {
                break;
            } else {
                accumulated_len += buf.len();
                remove += 1;
            }
        }

        let bufs = &mut bufs[remove..];
        if !bufs.is_empty() {
            bufs[0].0.advance(n - accumulated_len)
        }
        bufs
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> Deref for IoSliceMut<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> DerefMut for IoSliceMut<'a> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

/// A buffer type used with `Write::write_vectored`.
///
/// It is semantically a wrapper around an `&[u8]`, but is guaranteed to be
/// ABI compatible with the `iovec` type on Unix platforms and `WSABUF` on
/// Windows.
#[stable(feature = "iovec", since = "1.36.0")]
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct IoSlice<'a>(sys::io::IoSlice<'a>);

#[stable(feature = "iovec-send-sync", since = "1.44.0")]
unsafe impl<'a> Send for IoSlice<'a> {}

#[stable(feature = "iovec-send-sync", since = "1.44.0")]
unsafe impl<'a> Sync for IoSlice<'a> {}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> fmt::Debug for IoSlice<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), fmt)
    }
}

impl<'a> IoSlice<'a> {
    /// Creates a new `IoSlice` wrapping a byte slice.
    ///
    /// # Panics
    ///
    /// Panics on Windows if the slice is larger than 4GB.
    #[stable(feature = "iovec", since = "1.36.0")]
    #[inline]
    pub fn new(buf: &'a [u8]) -> IoSlice<'a> {
        IoSlice(sys::io::IoSlice::new(buf))
    }

    /// Advance the internal cursor of the slice.
    ///
    /// # Notes
    ///
    /// Elements in the slice may be modified if the cursor is not advanced to
    /// the end of the slice. For example if we have a slice of buffers with 2
    /// `IoSlice`s, both of length 8, and we advance the cursor by 10 bytes the
    /// first `IoSlice` will be untouched however the second will be modified to
    /// remove the first 2 bytes (10 - 8).
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(io_slice_advance)]
    ///
    /// use std::io::IoSlice;
    /// use std::ops::Deref;
    ///
    /// let buf1 = [1; 8];
    /// let buf2 = [2; 16];
    /// let buf3 = [3; 8];
    /// let mut bufs = &mut [
    ///     IoSlice::new(&buf1),
    ///     IoSlice::new(&buf2),
    ///     IoSlice::new(&buf3),
    /// ][..];
    ///
    /// // Mark 10 bytes as written.
    /// bufs = IoSlice::advance(bufs, 10);
    /// assert_eq!(bufs[0].deref(), [2; 14].as_ref());
    /// assert_eq!(bufs[1].deref(), [3; 8].as_ref());
    #[unstable(feature = "io_slice_advance", issue = "62726")]
    #[inline]
    pub fn advance<'b>(bufs: &'b mut [IoSlice<'a>], n: usize) -> &'b mut [IoSlice<'a>] {
        // Number of buffers to remove.
        let mut remove = 0;
        // Total length of all the to be removed buffers.
        let mut accumulated_len = 0;
        for buf in bufs.iter() {
            if accumulated_len + buf.len() > n {
                break;
            } else {
                accumulated_len += buf.len();
                remove += 1;
            }
        }

        let bufs = &mut bufs[remove..];
        if !bufs.is_empty() {
            bufs[0].0.advance(n - accumulated_len)
        }
        bufs
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> Deref for IoSlice<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// A type used to conditionally initialize buffers passed to `Read` methods.
#[unstable(feature = "read_initializer", issue = "42788")]
#[derive(Debug)]
pub struct Initializer(bool);

impl Initializer {
    /// Returns a new `Initializer` which will zero out buffers.
    #[unstable(feature = "read_initializer", issue = "42788")]
    #[inline]
    pub fn zeroing() -> Initializer {
        Initializer(true)
    }

    /// Returns a new `Initializer` which will not zero out buffers.
    ///
    /// # Safety
    ///
    /// This may only be called by `Read`ers which guarantee that they will not
    /// read from buffers passed to `Read` methods, and that the return value of
    /// the method accurately reflects the number of bytes that have been
    /// written to the head of the buffer.
    #[unstable(feature = "read_initializer", issue = "42788")]
    #[inline]
    pub unsafe fn nop() -> Initializer {
        Initializer(false)
    }

    /// Indicates if a buffer should be initialized.
    #[unstable(feature = "read_initializer", issue = "42788")]
    #[inline]
    pub fn should_initialize(&self) -> bool {
        self.0
    }

    /// Initializes a buffer if necessary.
    #[unstable(feature = "read_initializer", issue = "42788")]
    #[inline]
    pub fn initialize(&self, buf: &mut [u8]) {
        if self.should_initialize() {
            unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()) }
        }
    }
}

/// A trait for objects which are byte-oriented sinks.
///
/// Implementors of the `Write` trait are sometimes called 'writers'.
///
/// Writers are defined by two required methods, [`write`] and [`flush`]:
///
/// * The [`write`] method will attempt to write some data into the object,
///   returning how many bytes were successfully written.
///
/// * The [`flush`] method is useful for adaptors and explicit buffers
///   themselves for ensuring that all buffered data has been pushed out to the
///   'true sink'.
///
/// Writers are intended to be composable with one another. Many implementors
/// throughout [`std::io`] take and provide types which implement the `Write`
/// trait.
///
/// [`write`]: Write::write
/// [`flush`]: Write::flush
/// [`std::io`]: self
///
/// # Examples
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> std::io::Result<()> {
///     let data = b"some bytes";
///
///     let mut pos = 0;
///     let mut buffer = File::create("foo.txt")?;
///
///     while pos < data.len() {
///         let bytes_written = buffer.write(&data[pos..])?;
///         pos += bytes_written;
///     }
///     Ok(())
/// }
/// ```
///
/// The trait also provides convenience methods like [`write_all`], which calls
/// `write` in a loop until its entire input has been written.
///
/// [`write_all`]: Write::write_all
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(spotlight)]
pub trait Write {
    /// Write a buffer into this writer, returning how many bytes were written.
    ///
    /// This function will attempt to write the entire contents of `buf`, but
    /// the entire write may not succeed, or the write may also generate an
    /// error. A call to `write` represents *at most one* attempt to write to
    /// any wrapped object.
    ///
    /// Calls to `write` are not guaranteed to block waiting for data to be
    /// written, and a write which would otherwise block can be indicated through
    /// an [`Err`] variant.
    ///
    /// If the return value is [`Ok(n)`] then it must be guaranteed that
    /// `n <= buf.len()`. A return value of `0` typically means that the
    /// underlying object is no longer able to accept bytes and will likely not
    /// be able to in the future as well, or that the buffer provided is empty.
    ///
    /// # Errors
    ///
    /// Each call to `write` may generate an I/O error indicating that the
    /// operation could not be completed. If an error is returned then no bytes
    /// in the buffer were written to this writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be
    /// written to this writer.
    ///
    /// An error of the [`ErrorKind::Interrupted`] kind is non-fatal and the
    /// write operation should be retried if there is nothing else to do.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // Writes some prefix of the byte string, not necessarily all of it.
    ///     buffer.write(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`Ok(n)`]: Ok
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Like [`write`], except that it writes from a slice of buffers.
    ///
    /// Data is copied from each buffer in order, with the final buffer
    /// read from possibly being only partially consumed. This method must
    /// behave as a call to [`write`] with the buffers concatenated would.
    ///
    /// The default implementation calls [`write`] with either the first nonempty
    /// buffer provided, or an empty one if none exists.
    ///
    /// [`write`]: Write::write
    #[stable(feature = "iovec", since = "1.36.0")]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        default_write_vectored(|b| self.write(b), bufs)
    }

    /// Determines if this `Write`r has an efficient [`write_vectored`]
    /// implementation.
    ///
    /// If a `Write`r does not override the default [`write_vectored`]
    /// implementation, code using it may want to avoid the method all together
    /// and coalesce writes into a single buffer for higher performance.
    ///
    /// The default implementation returns `false`.
    ///
    /// [`write_vectored`]: Write::write_vectored
    #[unstable(feature = "can_vector", issue = "69941")]
    fn is_write_vectored(&self) -> bool {
        false
    }

    /// Flush this output stream, ensuring that all intermediately buffered
    /// contents reach their destination.
    ///
    /// # Errors
    ///
    /// It is considered an error if not all bytes could be written due to
    /// I/O errors or EOF being reached.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::io::BufWriter;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = BufWriter::new(File::create("foo.txt")?);
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     buffer.flush()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn flush(&mut self) -> Result<()>;

    /// Attempts to write an entire buffer into this writer.
    ///
    /// This method will continuously call [`write`] until there is no more data
    /// to be written or an error of non-[`ErrorKind::Interrupted`] kind is
    /// returned. This method will not return until the entire buffer has been
    /// successfully written or such an error occurs. The first error that is
    /// not of [`ErrorKind::Interrupted`] kind generated from this method will be
    /// returned.
    ///
    /// If the buffer contains no data, this will never call [`write`].
    ///
    /// # Errors
    ///
    /// This function will return the first error of
    /// non-[`ErrorKind::Interrupted`] kind that [`write`] returns.
    ///
    /// [`write`]: Write::write
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::new(ErrorKind::WriteZero, "failed to write whole buffer"));
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Attempts to write multiple buffers into this writer.
    ///
    /// This method will continuously call [`write_vectored`] until there is no
    /// more data to be written or an error of non-[`ErrorKind::Interrupted`]
    /// kind is returned. This method will not return until all buffers have
    /// been successfully written or such an error occurs. The first error that
    /// is not of [`ErrorKind::Interrupted`] kind generated from this method
    /// will be returned.
    ///
    /// If the buffer contains no data, this will never call [`write_vectored`].
    ///
    /// # Notes
    ///
    /// Unlike [`write_vectored`], this takes a *mutable* reference to
    /// a slice of [`IoSlice`]s, not an immutable one. That's because we need to
    /// modify the slice to keep track of the bytes already written.
    ///
    /// Once this function returns, the contents of `bufs` are unspecified, as
    /// this depends on how many calls to [`write_vectored`] were necessary. It is
    /// best to understand this function as taking ownership of `bufs` and to
    /// not use `bufs` afterwards. The underlying buffers, to which the
    /// [`IoSlice`]s point (but not the [`IoSlice`]s themselves), are unchanged and
    /// can be reused.
    ///
    /// [`write_vectored`]: Write::write_vectored
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(write_all_vectored)]
    /// # fn main() -> std::io::Result<()> {
    ///
    /// use std::io::{Write, IoSlice};
    ///
    /// let mut writer = Vec::new();
    /// let bufs = &mut [
    ///     IoSlice::new(&[1]),
    ///     IoSlice::new(&[2, 3]),
    ///     IoSlice::new(&[4, 5, 6]),
    /// ];
    ///
    /// writer.write_all_vectored(bufs)?;
    /// // Note: the contents of `bufs` is now undefined, see the Notes section.
    ///
    /// assert_eq!(writer, &[1, 2, 3, 4, 5, 6]);
    /// # Ok(()) }
    /// ```
    #[unstable(feature = "write_all_vectored", issue = "70436")]
    fn write_all_vectored(&mut self, mut bufs: &mut [IoSlice<'_>]) -> Result<()> {
        // Guarantee that bufs is empty if it contains no data,
        // to avoid calling write_vectored if there is no data to be written.
        bufs = IoSlice::advance(bufs, 0);
        while !bufs.is_empty() {
            match self.write_vectored(bufs) {
                Ok(0) => {
                    return Err(Error::new(ErrorKind::WriteZero, "failed to write whole buffer"));
                }
                Ok(n) => bufs = IoSlice::advance(bufs, n),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Writes a formatted string into this writer, returning any error
    /// encountered.
    ///
    /// This method is primarily used to interface with the
    /// [`format_args!()`] macro, but it is rare that this should
    /// explicitly be called. The [`write!()`] macro should be favored to
    /// invoke this method instead.
    ///
    /// This function internally uses the [`write_all`] method on
    /// this trait and hence will continuously write data so long as no errors
    /// are received. This also means that partial writes are not indicated in
    /// this signature.
    ///
    /// [`write_all`]: Write::write_all
    ///
    /// # Errors
    ///
    /// This function will return any I/O error reported while formatting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // this call
    ///     write!(buffer, "{:.*}", 2, 1.234567)?;
    ///     // turns into this:
    ///     buffer.write_fmt(format_args!("{:.*}", 2, 1.234567))?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<()> {
        // Create a shim which translates a Write to a fmt::Write and saves
        // off I/O errors. instead of discarding them
        struct Adaptor<'a, T: ?Sized + 'a> {
            inner: &'a mut T,
            error: Result<()>,
        }

        impl<T: Write + ?Sized> fmt::Write for Adaptor<'_, T> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                match self.inner.write_all(s.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        self.error = Err(e);
                        Err(fmt::Error)
                    }
                }
            }
        }

        let mut output = Adaptor { inner: self, error: Ok(()) };
        match fmt::write(&mut output, fmt) {
            Ok(()) => Ok(()),
            Err(..) => {
                // check if the error came from the underlying `Write` or not
                if output.error.is_err() {
                    output.error
                } else {
                    Err(Error::new(ErrorKind::Other, "formatter error"))
                }
            }
        }
    }

    /// Creates a "by reference" adaptor for this instance of `Write`.
    ///
    /// The returned adaptor also implements `Write` and will simply borrow this
    /// current writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     let reference = buffer.by_ref();
    ///
    ///     // we can use reference just like our original buffer
    ///     reference.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

/// The `Seek` trait provides a cursor which can be moved within a stream of
/// bytes.
///
/// The stream typically has a fixed size, allowing seeking relative to either
/// end or the current offset.
///
/// # Examples
///
/// [`File`]s implement `Seek`:
///
/// [`File`]: crate::fs::File
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
/// use std::fs::File;
/// use std::io::SeekFrom;
///
/// fn main() -> io::Result<()> {
///     let mut f = File::open("foo.txt")?;
///
///     // move the cursor 42 bytes from the start of the file
///     f.seek(SeekFrom::Start(42))?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Seek {
    /// Seek to an offset, in bytes, in a stream.
    ///
    /// A seek beyond the end of a stream is allowed, but behavior is defined
    /// by the implementation.
    ///
    /// If the seek operation completed successfully,
    /// this method returns the new position from the start of the stream.
    /// That position can be used later with [`SeekFrom::Start`].
    ///
    /// # Errors
    ///
    /// Seeking to a negative offset is considered an error.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    /// Returns the length of this stream (in bytes).
    ///
    /// This method is implemented using up to three seek operations. If this
    /// method returns successfully, the seek position is unchanged (i.e. the
    /// position before calling this method is the same as afterwards).
    /// However, if this method returns an error, the seek position is
    /// unspecified.
    ///
    /// If you need to obtain the length of *many* streams and you don't care
    /// about the seek position afterwards, you can reduce the number of seek
    /// operations by simply calling `seek(SeekFrom::End(0))` and using its
    /// return value (it is also the stream length).
    ///
    /// Note that length of a stream can change over time (for example, when
    /// data is appended to a file). So calling this method multiple times does
    /// not necessarily return the same length each time.
    ///
    /// # Example
    ///
    /// ```no_run
    /// #![feature(seek_stream_len)]
    /// use std::{
    ///     io::{self, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///
    ///     let len = f.stream_len()?;
    ///     println!("The file is currently {} bytes long", len);
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "seek_stream_len", issue = "59359")]
    fn stream_len(&mut self) -> Result<u64> {
        let old_pos = self.stream_position()?;
        let len = self.seek(SeekFrom::End(0))?;

        // Avoid seeking a third time when we were already at the end of the
        // stream. The branch is usually way cheaper than a seek operation.
        if old_pos != len {
            self.seek(SeekFrom::Start(old_pos))?;
        }

        Ok(len)
    }

    /// Returns the current seek position from the start of the stream.
    ///
    /// This is equivalent to `self.seek(SeekFrom::Current(0))`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::{
    ///     io::{self, BufRead, BufReader, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = BufReader::new(File::open("foo.txt")?);
    ///
    ///     let before = f.stream_position()?;
    ///     f.read_line(&mut String::new())?;
    ///     let after = f.stream_position()?;
    ///
    ///     println!("The first line was {} bytes long", after - before);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "seek_convenience", since = "1.51.0")]
    fn stream_position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

/// Enumeration of possible methods to seek within an I/O object.
///
/// It is used by the [`Seek`] trait.
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes.
    #[stable(feature = "rust1", since = "1.0.0")]
    Start(#[stable(feature = "rust1", since = "1.0.0")] u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    #[stable(feature = "rust1", since = "1.0.0")]
    End(#[stable(feature = "rust1", since = "1.0.0")] i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    #[stable(feature = "rust1", since = "1.0.0")]
    Current(#[stable(feature = "rust1", since = "1.0.0")] i64),
}

fn read_until<R: BufRead + ?Sized>(r: &mut R, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

/// A `BufRead` is a type of `Read`er which has an internal buffer, allowing it
/// to perform extra ways of reading.
///
/// For example, reading line-by-line is inefficient without using a buffer, so
/// if you want to read by line, you'll need `BufRead`, which includes a
/// [`read_line`] method as well as a [`lines`] iterator.
///
/// # Examples
///
/// A locked standard input implements `BufRead`:
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
///
/// let stdin = io::stdin();
/// for line in stdin.lock().lines() {
///     println!("{}", line.unwrap());
/// }
/// ```
///
/// If you have something that implements [`Read`], you can use the [`BufReader`
/// type][`BufReader`] to turn it into a `BufRead`.
///
/// For example, [`File`] implements [`Read`], but not `BufRead`.
/// [`BufReader`] to the rescue!
///
/// [`File`]: crate::fs::File
/// [`read_line`]: BufRead::read_line
/// [`lines`]: BufRead::lines
///
/// ```no_run
/// use std::io::{self, BufReader};
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> io::Result<()> {
///     let f = File::open("foo.txt")?;
///     let f = BufReader::new(f);
///
///     for line in f.lines() {
///         println!("{}", line.unwrap());
///     }
///
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait BufRead: Read {
    /// Returns the contents of the internal buffer, filling it with more data
    /// from the inner reader if it is empty.
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`consume`] method to function properly. When calling this
    /// method, none of the contents will be "read" in the sense that later
    /// calling `read` may return the same contents. As such, [`consume`] must
    /// be called with the number of bytes that are consumed from this buffer to
    /// ensure that the bytes are never returned twice.
    ///
    /// [`consume`]: BufRead::consume
    ///
    /// An empty buffer returned indicates that the stream has reached EOF.
    ///
    /// # Errors
    ///
    /// This function will return an I/O error if the underlying reader was
    /// read, but returned an error.
    ///
    /// # Examples
    ///
    /// A locked standard input implements `BufRead`:
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    ///
    /// let stdin = io::stdin();
    /// let mut stdin = stdin.lock();
    ///
    /// let buffer = stdin.fill_buf().unwrap();
    ///
    /// // work with buffer
    /// println!("{:?}", buffer);
    ///
    /// // ensure the bytes we worked with aren't returned again later
    /// let length = buffer.len();
    /// stdin.consume(length);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fill_buf(&mut self) -> Result<&[u8]>;

    /// Tells this buffer that `amt` bytes have been consumed from the buffer,
    /// so they should no longer be returned in calls to `read`.
    ///
    /// This function is a lower-level call. It needs to be paired with the
    /// [`fill_buf`] method to function properly. This function does
    /// not perform any I/O, it simply informs this object that some amount of
    /// its buffer, returned from [`fill_buf`], has been consumed and should
    /// no longer be returned. As such, this function may do odd things if
    /// [`fill_buf`] isn't called before calling it.
    ///
    /// The `amt` must be `<=` the number of bytes in the buffer returned by
    /// [`fill_buf`].
    ///
    /// # Examples
    ///
    /// Since `consume()` is meant to be used with [`fill_buf`],
    /// that method's example includes an example of `consume()`.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    #[stable(feature = "rust1", since = "1.0.0")]
    fn consume(&mut self, amt: usize);

    /// Read all bytes into `buf` until the delimiter `byte` or EOF is reached.
    ///
    /// This function will read bytes from the underlying stream until the
    /// delimiter or EOF is found. Once found, all bytes up to, and including,
    /// the delimiter (if found) will be appended to `buf`.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// This function is blocking and should be used carefully: it is possible for
    /// an attacker to continuously send bytes without ever sending the delimiter
    /// or EOF.
    ///
    /// # Errors
    ///
    /// This function will ignore all instances of [`ErrorKind::Interrupted`] and
    /// will otherwise return any errors returned by [`fill_buf`].
    ///
    /// If an I/O error is encountered then all bytes read so far will be
    /// present in `buf` and its length will have been adjusted appropriately.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # Examples
    ///
    /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to read all the bytes in a byte slice
    /// in hyphen delimited segments:
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"lorem-ipsum");
    /// let mut buf = vec![];
    ///
    /// // cursor is at 'l'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 6);
    /// assert_eq!(buf, b"lorem-");
    /// buf.clear();
    ///
    /// // cursor is at 'i'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 5);
    /// assert_eq!(buf, b"ipsum");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, b"");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<usize> {
        read_until(self, byte, buf)
    }

    /// Read all bytes until a newline (the `0xA` byte) is reached, and append
    /// them to the provided buffer.
    ///
    /// This function will read bytes from the underlying stream until the
    /// newline delimiter (the `0xA` byte) or EOF is found. Once found, all bytes
    /// up to, and including, the delimiter (if found) will be appended to
    /// `buf`.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// If this function returns [`Ok(0)`], the stream has reached EOF.
    ///
    /// This function is blocking and should be used carefully: it is possible for
    /// an attacker to continuously send bytes without ever sending a newline
    /// or EOF.
    ///
    /// [`Ok(0)`]: Ok
    ///
    /// # Errors
    ///
    /// This function has the same error semantics as [`read_until`] and will
    /// also return an error if the read bytes are not valid UTF-8. If an I/O
    /// error is encountered then `buf` may contain some bytes already read in
    /// the event that all data read so far was valid UTF-8.
    ///
    /// [`read_until`]: BufRead::read_until
    ///
    /// # Examples
    ///
    /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to read all the lines in a byte slice:
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"foo\nbar");
    /// let mut buf = String::new();
    ///
    /// // cursor is at 'f'
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 4);
    /// assert_eq!(buf, "foo\n");
    /// buf.clear();
    ///
    /// // cursor is at 'b'
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 3);
    /// assert_eq!(buf, "bar");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, "");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        // Note that we are not calling the `.read_until` method here, but
        // rather our hardcoded implementation. For more details as to why, see
        // the comments in `read_to_end`.
        append_to_string(buf, |b| read_until(self, b'\n', b))
    }

    /// Returns an iterator over the contents of this reader split on the byte
    /// `byte`.
    ///
    /// The iterator returned from this function will return instances of
    /// [`io::Result`]`<`[`Vec<u8>`]`>`. Each vector returned will *not* have
    /// the delimiter byte at the end.
    ///
    /// This function will yield errors whenever [`read_until`] would have
    /// also yielded an error.
    ///
    /// [`io::Result`]: self::Result
    /// [`read_until`]: BufRead::read_until
    ///
    /// # Examples
    ///
    /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to iterate over all hyphen delimited
    /// segments in a byte slice
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem-ipsum-dolor");
    ///
    /// let mut split_iter = cursor.split(b'-').map(|l| l.unwrap());
    /// assert_eq!(split_iter.next(), Some(b"lorem".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"ipsum".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"dolor".to_vec()));
    /// assert_eq!(split_iter.next(), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split(self, byte: u8) -> Split<Self>
    where
        Self: Sized,
    {
        Split { buf: self, delim: byte }
    }

    /// Returns an iterator over the lines of this reader.
    ///
    /// The iterator returned from this function will yield instances of
    /// [`io::Result`]`<`[`String`]`>`. Each string returned will *not* have a newline
    /// byte (the `0xA` byte) or `CRLF` (`0xD`, `0xA` bytes) at the end.
    ///
    /// [`io::Result`]: self::Result
    ///
    /// # Examples
    ///
    /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
    /// this example, we use [`Cursor`] to iterate over all the lines in a byte
    /// slice.
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines_iter = cursor.lines().map(|l| l.unwrap());
    /// assert_eq!(lines_iter.next(), Some(String::from("lorem")));
    /// assert_eq!(lines_iter.next(), Some(String::from("ipsum")));
    /// assert_eq!(lines_iter.next(), Some(String::from("dolor")));
    /// assert_eq!(lines_iter.next(), None);
    /// ```
    ///
    /// # Errors
    ///
    /// Each line of the iterator has the same error semantics as [`BufRead::read_line`].
    #[stable(feature = "rust1", since = "1.0.0")]
    fn lines(self) -> Lines<Self>
    where
        Self: Sized,
    {
        Lines { buf: self }
    }
}

/// Adaptor to chain together two readers.
///
/// This struct is generally created by calling [`chain`] on a reader.
/// Please see the documentation of [`chain`] for more details.
///
/// [`chain`]: Read::chain
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Chain<T, U> {
    first: T,
    second: U,
    done_first: bool,
}

impl<T, U> Chain<T, U> {
    /// Consumes the `Chain`, returning the wrapped readers.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn into_inner(self) -> (T, U) {
        (self.first, self.second)
    }

    /// Gets references to the underlying readers in this `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.first, &self.second)
    }

    /// Gets mutable references to the underlying readers in this `Chain`.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let mut chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.first, &mut self.second)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: fmt::Debug, U: fmt::Debug> fmt::Debug for Chain<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain").field("t", &self.first).field("u", &self.second).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Read, U: Read> Read for Chain<T, U> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.done_first {
            match self.first.read(buf)? {
                0 if !buf.is_empty() => self.done_first = true,
                n => return Ok(n),
            }
        }
        self.second.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        if !self.done_first {
            match self.first.read_vectored(bufs)? {
                0 if bufs.iter().any(|b| !b.is_empty()) => self.done_first = true,
                n => return Ok(n),
            }
        }
        self.second.read_vectored(bufs)
    }

    unsafe fn initializer(&self) -> Initializer {
        let initializer = self.first.initializer();
        if initializer.should_initialize() {
            initializer
        } else {
            self.second.initializer()
        }
    }
}

#[stable(feature = "chain_bufread", since = "1.9.0")]
impl<T: BufRead, U: BufRead> BufRead for Chain<T, U> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if !self.done_first {
            match self.first.fill_buf()? {
                buf if buf.is_empty() => {
                    self.done_first = true;
                }
                buf => return Ok(buf),
            }
        }
        self.second.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        if !self.done_first {
            self.first.consume(amt)
        } else {
            self.second.consume(amt)
        }
    }
}

/// Reader adaptor which limits the bytes read from an underlying reader.
///
/// This struct is generally created by calling [`take`] on a reader.
/// Please see the documentation of [`take`] for more details.
///
/// [`take`]: Read::take
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    limit: u64,
}

impl<T> Take<T> {
    /// Returns the number of bytes that can be read before this instance will
    /// return EOF.
    ///
    /// # Note
    ///
    /// This instance may reach `EOF` after reading fewer bytes than indicated by
    /// this method if the underlying [`Read`] instance reaches EOF.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // read at most five bytes
    ///     let handle = f.take(5);
    ///
    ///     println!("limit: {}", handle.limit());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Sets the number of bytes that can be read before this instance will
    /// return EOF. This is the same as constructing a new `Take` instance, so
    /// the amount of bytes read and the previous limit value don't matter when
    /// calling this method.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // read at most five bytes
    ///     let mut handle = f.take(5);
    ///     handle.set_limit(10);
    ///
    ///     assert_eq!(handle.limit(), 10);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "take_set_limit", since = "1.27.0")]
    pub fn set_limit(&mut self, limit: u64) {
        self.limit = limit;
    }

    /// Consumes the `Take`, returning the wrapped reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "io_take_into_inner", since = "1.15.0")]
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying reader.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the internal limit of this
    /// `Take`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Read> Read for Take<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // Don't call into inner reader at all at EOF because it may still block
        if self.limit == 0 {
            return Ok(0);
        }

        let max = cmp::min(buf.len() as u64, self.limit) as usize;
        let n = self.inner.read(&mut buf[..max])?;
        self.limit -= n as u64;
        Ok(n)
    }

    unsafe fn initializer(&self) -> Initializer {
        self.inner.initializer()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        // Pass in a reservation_size closure that respects the current value
        // of limit for each read. If we hit the read limit, this prevents the
        // final zero-byte read from allocating again.
        read_to_end_with_reservation(self, buf, |self_| cmp::min(self_.limit, 32) as usize)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: BufRead> BufRead for Take<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        // Don't call into inner reader at all at EOF because it may still block
        if self.limit == 0 {
            return Ok(&[]);
        }

        let buf = self.inner.fill_buf()?;
        let cap = cmp::min(buf.len() as u64, self.limit) as usize;
        Ok(&buf[..cap])
    }

    fn consume(&mut self, amt: usize) {
        // Don't let callers reset the limit by passing an overlarge value
        let amt = cmp::min(amt as u64, self.limit) as usize;
        self.limit -= amt as u64;
        self.inner.consume(amt);
    }
}

/// An iterator over `u8` values of a reader.
///
/// This struct is generally created by calling [`bytes`] on a reader.
/// Please see the documentation of [`bytes`] for more details.
///
/// [`bytes`]: Read::bytes
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Bytes<R> {
    inner: R,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: Read + SizeHint> Iterator for Bytes<R> {
    type Item = Result<u8>;

    fn next(&mut self) -> Option<Result<u8>> {
        let mut byte = 0;
        loop {
            return match self.inner.read(slice::from_mut(&mut byte)) {
                Ok(0) => None,
                Ok(..) => Some(Ok(byte)),
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => Some(Err(e)),
            };
        }
    }

    default fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "bufreader_size_hint", since = "1.51.0")]
trait SizeHint {
    fn lower_bound(&self) -> usize;

    fn upper_bound(&self) -> Option<usize> {
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.lower_bound(), self.upper_bound())
    }
}

#[stable(feature = "bufreader_size_hint", since = "1.51.0")]
impl<T> SizeHint for T {
    fn lower_bound(&self) -> usize {
        0
    }
}

#[stable(feature = "bufreader_size_hint", since = "1.51.0")]
impl<T> SizeHint for BufReader<T> {
    fn lower_bound(&self) -> usize {
        self.buffer().len()
    }
}

/// An iterator over the contents of an instance of `BufRead` split on a
/// particular byte.
///
/// This struct is generally created by calling [`split`] on a `BufRead`.
/// Please see the documentation of [`split`] for more details.
///
/// [`split`]: BufRead::split
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Split<B> {
    buf: B,
    delim: u8,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead> Iterator for Split<B> {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Result<Vec<u8>>> {
        let mut buf = Vec::new();
        match self.buf.read_until(self.delim, &mut buf) {
            Ok(0) => None,
            Ok(_n) => {
                if buf[buf.len() - 1] == self.delim {
                    buf.pop();
                }
                Some(Ok(buf))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// An iterator over the lines of an instance of `BufRead`.
///
/// This struct is generally created by calling [`lines`] on a `BufRead`.
/// Please see the documentation of [`lines`] for more details.
///
/// [`lines`]: BufRead::lines
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Lines<B> {
    buf: B,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead> Iterator for Lines<B> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let mut buf = String::new();
        match self.buf.read_line(&mut buf) {
            Ok(0) => None,
            Ok(_n) => {
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Some(Ok(buf))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
