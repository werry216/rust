#![deny(unsafe_op_in_unsafe_fn)]

use crate::io::{self, IoSlice, IoSliceMut};
use crate::mem::ManuallyDrop;
use crate::sys::fd::WasiFd;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin
    }

    #[inline]
    pub fn as_raw_fd(&self) -> u32 {
        0
    }
}

impl io::Read for Stdin {
    fn read(&mut self, data: &mut [u8]) -> io::Result<usize> {
        self.read_vectored(&mut [IoSliceMut::new(data)])
    }

    fn read_vectored(&mut self, data: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        ManuallyDrop::new(unsafe { WasiFd::from_raw(self.as_raw_fd()) }).read(data)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        true
    }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout
    }

    #[inline]
    pub fn as_raw_fd(&self) -> u32 {
        1
    }
}

impl io::Write for Stdout {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_vectored(&[IoSlice::new(data)])
    }

    fn write_vectored(&mut self, data: &[IoSlice<'_>]) -> io::Result<usize> {
        ManuallyDrop::new(unsafe { WasiFd::from_raw(self.as_raw_fd()) }).write(data)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr
    }

    #[inline]
    pub fn as_raw_fd(&self) -> u32 {
        2
    }
}

impl io::Write for Stderr {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_vectored(&[IoSlice::new(data)])
    }

    fn write_vectored(&mut self, data: &[IoSlice<'_>]) -> io::Result<usize> {
        ManuallyDrop::new(unsafe { WasiFd::from_raw(self.as_raw_fd()) }).write(data)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = crate::sys_common::io::DEFAULT_BUF_SIZE;

pub fn is_ebadf(err: &io::Error) -> bool {
    err.raw_os_error() == Some(wasi::ERRNO_BADF.into())
}

pub fn panic_output() -> Option<impl io::Write> {
    Some(Stderr::new())
}
