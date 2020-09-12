use crate::convert::TryFrom;
use crate::io::{self, IoSliceMut};
use crate::marker::PhantomData;
use crate::mem::{size_of, zeroed};
use crate::os::unix::io::RawFd;
use crate::path::Path;
use crate::ptr::{null_mut, read_unaligned};
use crate::slice::from_raw_parts;
use crate::sys::unix::ext::net::addr::{sockaddr_un, SocketAddr};
use crate::sys::unix::net::Socket;

pub(super) fn recv_vectored_with_ancillary_from(
    socket: &Socket,
    bufs: &mut [IoSliceMut<'_>],
    ancillary: &mut SocketAncillary<'_>,
) -> io::Result<(usize, bool, io::Result<SocketAddr>)> {
    unsafe {
        let mut msg_name: libc::sockaddr_un = zeroed();

        let mut msg = libc::msghdr {
            msg_name: &mut msg_name as *mut _ as *mut _,
            msg_namelen: size_of::<libc::sockaddr_un>() as libc::socklen_t,
            msg_iov: bufs.as_mut_ptr().cast(),
            msg_iovlen: bufs.len(),
            msg_control: ancillary.buffer.as_mut_ptr().cast(),
            msg_controllen: ancillary.buffer.len(),
            msg_flags: 0,
        };

        let count = socket.recv_msg(&mut msg)?;

        ancillary.length = msg.msg_controllen;
        ancillary.truncated = msg.msg_flags & libc::MSG_CTRUNC == libc::MSG_CTRUNC;

        let truncated = msg.msg_flags & libc::MSG_TRUNC == libc::MSG_TRUNC;
        let addr = SocketAddr::from_parts(msg_name, msg.msg_namelen);

        Ok((count, truncated, addr))
    }
}

pub(super) fn send_vectored_with_ancillary_to(
    socket: &Socket,
    path: Option<&Path>,
    bufs: &mut [IoSliceMut<'_>],
    ancillary: &mut SocketAncillary<'_>,
) -> io::Result<usize> {
    unsafe {
        let (mut msg_name, msg_namelen) =
            if let Some(path) = path { sockaddr_un(path)? } else { (zeroed(), 0) };

        let mut msg = libc::msghdr {
            msg_name: &mut msg_name as *mut _ as *mut _,
            msg_namelen,
            msg_iov: bufs.as_mut_ptr().cast(),
            msg_iovlen: bufs.len(),
            msg_control: ancillary.buffer.as_mut_ptr().cast(),
            msg_controllen: ancillary.length,
            msg_flags: 0,
        };

        ancillary.truncated = false;

        socket.send_msg(&mut msg)
    }
}

fn add_to_ancillary_data<T>(
    buffer: &mut [u8],
    length: &mut usize,
    source: &[T],
    cmsg_level: libc::c_int,
    cmsg_type: libc::c_int,
) -> bool {
    let source_len = if let Some(source_len) = source.len().checked_mul(size_of::<T>()) {
        if let Ok(source_len) = u32::try_from(source_len) {
            source_len
        } else {
            return false;
        }
    } else {
        return false;
    };

    unsafe {
        let additional_space = libc::CMSG_SPACE(source_len) as usize;

        let new_length = if let Some(new_length) = additional_space.checked_add(*length) {
            new_length
        } else {
            return false;
        };

        if new_length > buffer.len() {
            return false;
        }

        buffer[*length..new_length].fill(0);

        *length = new_length;

        let msg = libc::msghdr {
            msg_name: null_mut(),
            msg_namelen: 0,
            msg_iov: null_mut(),
            msg_iovlen: 0,
            msg_control: buffer.as_mut_ptr().cast(),
            msg_controllen: *length,
            msg_flags: 0,
        };

        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        let mut previous_cmsg = cmsg;
        while !cmsg.is_null() {
            previous_cmsg = cmsg;
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }

        if previous_cmsg.is_null() {
            return false;
        }

        (*previous_cmsg).cmsg_level = cmsg_level;
        (*previous_cmsg).cmsg_type = cmsg_type;
        (*previous_cmsg).cmsg_len = libc::CMSG_LEN(source_len) as usize;

        let data = libc::CMSG_DATA(previous_cmsg).cast();

        libc::memcpy(data, source.as_ptr().cast(), source_len as usize);
    }
    true
}

struct AncillaryDataIter<'a, T> {
    data: &'a [u8],
    phantom: PhantomData<T>,
}

impl<'a, T> AncillaryDataIter<'a, T> {
    /// Create `AncillaryDataIter` struct to iterate through the data unit in the control message.
    ///
    /// # Safety
    ///
    /// `data` must contain a valid control message.
    unsafe fn new(data: &'a [u8]) -> AncillaryDataIter<'a, T> {
        AncillaryDataIter { data, phantom: PhantomData }
    }
}

impl<'a, T> Iterator for AncillaryDataIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if size_of::<T>() <= self.data.len() {
            unsafe {
                let unit = read_unaligned(self.data.as_ptr().cast());
                self.data = &self.data[size_of::<T>()..];
                Some(unit)
            }
        } else {
            None
        }
    }
}

/// Unix credential.
#[cfg(any(
    doc,
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_env = "uclibc",
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
#[derive(Clone)]
pub struct UCred(libc::ucred);

#[cfg(any(
    doc,
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_env = "uclibc",
))]
impl UCred {
    /// Create a Unix credential struct.
    ///
    /// PID, UID and GID is set to 0.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn new() -> UCred {
        UCred(libc::ucred { pid: 0, uid: 0, gid: 0 })
    }

    /// Set the PID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn set_pid(&mut self, pid: i32) {
        self.0.pid = pid;
    }

    /// Get the current PID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn get_pid(&self) -> i32 {
        self.0.pid
    }

    /// Set the UID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn set_uid(&mut self, uid: u32) {
        self.0.uid = uid;
    }

    /// Get the current UID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn get_uid(&self) -> u32 {
        self.0.uid
    }

    /// Set the GID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn set_gid(&mut self, gid: u32) {
        self.0.gid = gid;
    }

    /// Get the current GID.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn get_gid(&self) -> u32 {
        self.0.gid
    }
}

/// This control message contains file descriptors.
///
/// The level is equal to `SOL_SOCKET` and the type is equal to `SCM_RIGHTS`.
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
pub struct ScmRights<'a>(AncillaryDataIter<'a, RawFd>);

#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
impl<'a> Iterator for ScmRights<'a> {
    type Item = RawFd;

    fn next(&mut self) -> Option<RawFd> {
        self.0.next()
    }
}

/// This control message contains unix credentials.
///
/// The level is equal to `SOL_SOCKET` and the type is equal to `SCM_CREDENTIALS` or `SCM_CREDS`.
#[cfg(any(
    doc,
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_env = "uclibc",
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::ucred>);

#[cfg(any(
    doc,
    target_os = "android",
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_env = "uclibc",
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
impl<'a> Iterator for ScmCredentials<'a> {
    type Item = UCred;

    fn next(&mut self) -> Option<UCred> {
        Some(UCred(self.0.next()?))
    }
}

/// The error type which is returned from parsing the type a control message.
#[non_exhaustive]
#[derive(Debug)]
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
pub enum AncillaryError {
    Unknown { cmsg_level: i32, cmsg_type: i32 },
}

/// This enum represent one control message of variable type.
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
pub enum AncillaryData<'a> {
    ScmRights(ScmRights<'a>),
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "dragonfly",
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_env = "uclibc",
    ))]
    ScmCredentials(ScmCredentials<'a>),
}

impl<'a> AncillaryData<'a> {
    /// Create a `AncillaryData::ScmRights` variant.
    ///
    /// # Safety
    ///
    /// `data` must contain a valid control message and the control message must be type of
    /// `SOL_SOCKET` and level of `SCM_RIGHTS`.
    unsafe fn as_rights(data: &'a [u8]) -> Self {
        let ancillary_data_iter = AncillaryDataIter::new(data);
        let scm_rights = ScmRights(ancillary_data_iter);
        AncillaryData::ScmRights(scm_rights)
    }

    /// Create a `AncillaryData::ScmCredentials` variant.
    ///
    /// # Safety
    ///
    /// `data` must contain a valid control message and the control message must be type of
    /// `SOL_SOCKET` and level of `SCM_CREDENTIALS` or `SCM_CREDENTIALS`.
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "dragonfly",
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_env = "uclibc",
    ))]
    unsafe fn as_credentials(data: &'a [u8]) -> Self {
        let ancillary_data_iter = AncillaryDataIter::new(data);
        let scm_credentials = ScmCredentials(ancillary_data_iter);
        AncillaryData::ScmCredentials(scm_credentials)
    }

    fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Result<Self, AncillaryError> {
        unsafe {
            let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
            let data_len = (*cmsg).cmsg_len - cmsg_len_zero;
            let data = libc::CMSG_DATA(cmsg).cast();
            let data = from_raw_parts(data, data_len);

            match (*cmsg).cmsg_level {
                libc::SOL_SOCKET => match (*cmsg).cmsg_type {
                    libc::SCM_RIGHTS => Ok(AncillaryData::as_rights(data)),
                    #[cfg(any(
                        target_os = "android",
                        target_os = "emscripten",
                        target_os = "fuchsia",
                        target_os = "linux",
                        target_env = "uclibc",
                    ))]
                    libc::SCM_CREDENTIALS => Ok(AncillaryData::as_credentials(data)),
                    #[cfg(any(
                        target_os = "dragonfly",
                        target_os = "freebsd",
                        target_os = "ios",
                        target_os = "macos",
                        target_os = "netbsd",
                        target_os = "openbsd",
                    ))]
                    libc::SCM_CREDS => Ok(AncillaryData::as_credentials(data)),
                    cmsg_type => {
                        Err(AncillaryError::Unknown { cmsg_level: libc::SOL_SOCKET, cmsg_type })
                    }
                },
                cmsg_level => {
                    Err(AncillaryError::Unknown { cmsg_level, cmsg_type: (*cmsg).cmsg_type })
                }
            }
        }
    }
}

/// This struct is used to iterate through the control messages.
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
pub struct Messages<'a> {
    buffer: &'a [u8],
    current: Option<&'a libc::cmsghdr>,
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
impl<'a> Iterator for Messages<'a> {
    type Item = Result<AncillaryData<'a>, AncillaryError>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let msg = libc::msghdr {
                msg_name: null_mut(),
                msg_namelen: 0,
                msg_iov: null_mut(),
                msg_iovlen: 0,
                msg_control: self.buffer.as_ptr() as *mut _,
                msg_controllen: self.buffer.len(),
                msg_flags: 0,
            };

            let cmsg = if let Some(current) = self.current {
                libc::CMSG_NXTHDR(&msg, current)
            } else {
                libc::CMSG_FIRSTHDR(&msg)
            };

            let cmsg = cmsg.as_ref()?;
            self.current = Some(cmsg);
            let ancillary_result = AncillaryData::try_from_cmsghdr(cmsg);
            Some(ancillary_result)
        }
    }
}

/// A Unix socket Ancillary data struct.
///
/// # Example
/// ```no_run
/// #![feature(unix_socket_ancillary_data)]
/// use std::os::unix::net::{UnixStream, SocketAncillary, AncillaryData};
/// use std::io::IoSliceMut;
///
/// fn main() -> std::io::Result<()> {
///     let sock = UnixStream::connect("/tmp/sock")?;
///
///     let mut fds = [0; 8];
///     let mut ancillary_buffer = [0; 128];
///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
///
///     let mut buf = [1; 8];
///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
///
///     for ancillary_result in ancillary.messages() {
///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
///             for fd in scm_rights {
///                 println!("receive file descriptor: {}", fd);
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
#[derive(Debug)]
pub struct SocketAncillary<'a> {
    buffer: &'a mut [u8],
    length: usize,
    truncated: bool,
}

impl<'a> SocketAncillary<'a> {
    /// Create an ancillary data with the given buffer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #![allow(unused_mut)]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::SocketAncillary;
    /// let mut ancillary_buffer = [0; 128];
    /// let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn new(buffer: &'a mut [u8]) -> Self {
        SocketAncillary { buffer, length: 0, truncated: false }
    }

    /// Returns the capacity of the buffer.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the number of used bytes.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns the iterator of the control messages.
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn messages(&self) -> Messages<'_> {
        Messages { buffer: &self.buffer[..self.length], current: None }
    }

    /// Is `true` if during a recv operation the ancillary was truncated.
    ///
    /// # Example
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///
    ///     let mut buf = [1; 8];
    ///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///
    ///     println!("Is truncated: {}", ancillary.truncated());
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Add file descriptors to the ancillary data.
    ///
    /// The function returns `true` if there was enough space in the buffer.
    /// If there was not enough space then no file descriptors was appended.
    /// Technically, that means this operation adds a control message with the level `SOL_SOCKET`
    /// and type `SCM_RIGHTS`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary};
    /// use std::os::unix::io::AsRawFd;
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     ancillary.add_fds(&[sock.as_raw_fd()][..]);
    ///
    ///     let mut buf = [1; 8];
    ///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
    ///     sock.send_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn add_fds(&mut self, fds: &[RawFd]) -> bool {
        self.truncated = false;
        add_to_ancillary_data(
            &mut self.buffer,
            &mut self.length,
            fds,
            libc::SOL_SOCKET,
            libc::SCM_RIGHTS,
        )
    }

    /// Add credentials to the ancillary data.
    ///
    /// The function returns `true` if there was enough space in the buffer.
    /// If there was not enough space then no credentials was appended.
    /// Technically, that means this operation adds a control message with the level `SOL_SOCKET`
    /// and type `SCM_CREDENTIALS` or `SCM_CREDS`.
    ///
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "dragonfly",
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_env = "uclibc",
    ))]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn add_creds(&mut self, creds: &[UCred]) -> bool {
        self.truncated = false;
        add_to_ancillary_data(
            &mut self.buffer,
            &mut self.length,
            creds,
            libc::SOL_SOCKET,
            #[cfg(any(
                target_os = "android",
                target_os = "emscripten",
                target_os = "fuchsia",
                target_os = "linux",
                target_env = "uclibc",
            ))]
            libc::SCM_CREDENTIALS,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            libc::SCM_CREDS,
        )
    }

    /// Clears the ancillary data, removing all values.
    ///
    /// # Example
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut fds1 = [0; 8];
    ///     let mut fds2 = [0; 8];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///
    ///     let mut buf = [1; 8];
    ///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
    ///
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {}", fd);
    ///             }
    ///         }
    ///     }
    ///
    ///     ancillary.clear();
    ///
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {}", fd);
    ///             }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "none")]
    pub fn clear(&mut self) {
        self.length = 0;
        self.truncated = false;
    }
}
