#![unstable(reason = "not public", issue = "none", feature = "fd")]

use crate::cmp;
use crate::io::{self, Initializer, IoSlice, IoSliceMut, Read};
use crate::mem;
use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sys::cvt;
use crate::sys_common::AsInner;

use libc::{c_int, c_void};

#[derive(Debug)]
pub struct FileDesc {
    fd: c_int,
}

// The maximum read limit on most POSIX-like systems is `SSIZE_MAX`,
// with the man page quoting that if the count of bytes to read is
// greater than `SSIZE_MAX` the result is "unspecified".
//
// On macOS, however, apparently the 64-bit libc is either buggy or
// intentionally showing odd behavior by rejecting any read with a size
// larger than or equal to INT_MAX. To handle both of these the read
// size is capped on both platforms.
#[cfg(target_os = "macos")]
const READ_LIMIT: usize = c_int::MAX as usize - 1;
#[cfg(not(target_os = "macos"))]
const READ_LIMIT: usize = libc::ssize_t::MAX as usize;

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn max_iov() -> usize {
    static LIM: AtomicUsize = AtomicUsize::new(0);

    let mut lim = LIM.load(Ordering::Relaxed);
    if lim == 0 {
        let ret = unsafe {
            libc::sysconf(
                #[cfg(target_os = "linux")]
                libc::_SC_IOV_MAX,
                #[cfg(target_os = "macos")]
                libc::_SC_UIO_MAXIOV,
            )
        };

        // 1024 is the default value on modern Linux systems
        // and hopefully more useful than `c_int::MAX`.
        lim = if ret > 0 { ret as usize } else { 1024 };
        LIM.store(lim, Ordering::Relaxed);
    }

    lim
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn max_iov() -> usize {
    c_int::MAX as usize
}

impl FileDesc {
    pub fn new(fd: c_int) -> FileDesc {
        FileDesc { fd }
    }

    pub fn raw(&self) -> c_int {
        self.fd
    }

    /// Extracts the actual file descriptor without closing it.
    pub fn into_raw(self) -> c_int {
        let fd = self.fd;
        mem::forget(self);
        fd
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::read(self.fd, buf.as_mut_ptr() as *mut c_void, cmp::min(buf.len(), READ_LIMIT))
        })?;
        Ok(ret as usize)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::readv(
                self.fd,
                bufs.as_ptr() as *const libc::iovec,
                cmp::min(bufs.len(), max_iov()) as c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        true
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut me = self;
        (&mut me).read_to_end(buf)
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        #[cfg(target_os = "android")]
        use super::android::cvt_pread64;

        #[cfg(not(target_os = "android"))]
        unsafe fn cvt_pread64(
            fd: c_int,
            buf: *mut c_void,
            count: usize,
            offset: i64,
        ) -> io::Result<isize> {
            #[cfg(not(target_os = "linux"))]
            use libc::pread as pread64;
            #[cfg(target_os = "linux")]
            use libc::pread64;
            cvt(pread64(fd, buf, count, offset))
        }

        unsafe {
            cvt_pread64(
                self.fd,
                buf.as_mut_ptr() as *mut c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as i64,
            )
            .map(|n| n as usize)
        }
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::write(self.fd, buf.as_ptr() as *const c_void, cmp::min(buf.len(), READ_LIMIT))
        })?;
        Ok(ret as usize)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let ret = cvt(unsafe {
            libc::writev(
                self.fd,
                bufs.as_ptr() as *const libc::iovec,
                cmp::min(bufs.len(), max_iov()) as c_int,
            )
        })?;
        Ok(ret as usize)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        true
    }

    pub fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        #[cfg(target_os = "android")]
        use super::android::cvt_pwrite64;

        #[cfg(not(target_os = "android"))]
        unsafe fn cvt_pwrite64(
            fd: c_int,
            buf: *const c_void,
            count: usize,
            offset: i64,
        ) -> io::Result<isize> {
            #[cfg(not(target_os = "linux"))]
            use libc::pwrite as pwrite64;
            #[cfg(target_os = "linux")]
            use libc::pwrite64;
            cvt(pwrite64(fd, buf, count, offset))
        }

        unsafe {
            cvt_pwrite64(
                self.fd,
                buf.as_ptr() as *const c_void,
                cmp::min(buf.len(), READ_LIMIT),
                offset as i64,
            )
            .map(|n| n as usize)
        }
    }

    #[cfg(target_os = "linux")]
    pub fn get_cloexec(&self) -> io::Result<bool> {
        unsafe { Ok((cvt(libc::fcntl(self.fd, libc::F_GETFD))? & libc::FD_CLOEXEC) != 0) }
    }

    #[cfg(not(any(
        target_env = "newlib",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "l4re",
        target_os = "linux",
        target_os = "haiku",
        target_os = "redox"
    )))]
    pub fn set_cloexec(&self) -> io::Result<()> {
        unsafe {
            cvt(libc::ioctl(self.fd, libc::FIOCLEX))?;
            Ok(())
        }
    }
    #[cfg(any(
        target_env = "newlib",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "l4re",
        target_os = "linux",
        target_os = "haiku",
        target_os = "redox"
    ))]
    pub fn set_cloexec(&self) -> io::Result<()> {
        unsafe {
            let previous = cvt(libc::fcntl(self.fd, libc::F_GETFD))?;
            let new = previous | libc::FD_CLOEXEC;
            if new != previous {
                cvt(libc::fcntl(self.fd, libc::F_SETFD, new))?;
            }
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe {
            let v = nonblocking as c_int;
            cvt(libc::ioctl(self.fd, libc::FIONBIO, &v))?;
            Ok(())
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe {
            let previous = cvt(libc::fcntl(self.fd, libc::F_GETFL))?;
            let new = if nonblocking {
                previous | libc::O_NONBLOCK
            } else {
                previous & !libc::O_NONBLOCK
            };
            if new != previous {
                cvt(libc::fcntl(self.fd, libc::F_SETFL, new))?;
            }
            Ok(())
        }
    }

    pub fn duplicate(&self) -> io::Result<FileDesc> {
        // We want to atomically duplicate this file descriptor and set the
        // CLOEXEC flag, and currently that's done via F_DUPFD_CLOEXEC. This
        // is a POSIX flag that was added to Linux in 2.6.24.
        let fd = cvt(unsafe { libc::fcntl(self.raw(), libc::F_DUPFD_CLOEXEC, 0) })?;
        Ok(FileDesc::new(fd))
    }
}

impl<'a> Read for &'a FileDesc {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    unsafe fn initializer(&self) -> Initializer {
        Initializer::nop()
    }
}

impl AsInner<c_int> for FileDesc {
    fn as_inner(&self) -> &c_int {
        &self.fd
    }
}

impl Drop for FileDesc {
    fn drop(&mut self) {
        // Note that errors are ignored when closing a file descriptor. The
        // reason for this is that if an error occurs we don't actually know if
        // the file descriptor was closed or not, and if we retried (for
        // something like EINTR), we might close another valid file descriptor
        // opened after we closed ours.
        let _ = unsafe { libc::close(self.fd) };
    }
}

#[cfg(test)]
mod tests {
    use super::{FileDesc, IoSlice};

    #[test]
    fn limit_vector_count() {
        let stdout = FileDesc { fd: 1 };
        let bufs = (0..1500).map(|_| IoSlice::new(&[])).collect::<Vec<_>>();

        assert!(stdout.write_vectored(&bufs).is_ok());
    }
}
