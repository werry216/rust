//! WASI-specific extensions to primitives in the `std::fs` module.

#![unstable(feature = "wasi_ext", issue = "none")]

use crate::fs::{self, File, Metadata, OpenOptions};
use crate::io::{self, IoSlice, IoSliceMut};
use crate::path::{Path, PathBuf};
use crate::sys::fs::osstr2str;
use crate::sys_common::{AsInner, AsInnerMut, FromInner};

/// WASI-specific extensions to [`File`].
///
/// [`File`]: ../../../../std/fs/struct.File.html
pub trait FileExt {
    /// Reads a number of bytes starting from a given offset.
    ///
    /// Returns the number of bytes read.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// Note that similar to [`File::read`], it is not an error to return with a
    /// short read.
    ///
    /// [`File::read`]: ../../../../std/fs/struct.File.html#method.read
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let bufs = &mut [IoSliceMut::new(buf)];
        self.read_vectored_at(bufs, offset)
    }

    /// Reads a number of bytes starting from a given offset.
    ///
    /// Returns the number of bytes read.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// Note that similar to [`File::read_vectored`], it is not an error to
    /// return with a short read.
    ///
    /// [`File::read`]: ../../../../std/fs/struct.File.html#method.read_vectored
    fn read_vectored_at(&self, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize>;

    /// Reads the exact number of byte required to fill `buf` from the given offset.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// Similar to [`Read::read_exact`] but uses [`read_at`] instead of `read`.
    ///
    /// [`Read::read_exact`]: ../../../../std/io/trait.Read.html#method.read_exact
    /// [`read_at`]: #tymethod.read_at
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
    /// [`ErrorKind::Interrupted`]: ../../../../std/io/enum.ErrorKind.html#variant.Interrupted
    /// [`ErrorKind::UnexpectedEof`]: ../../../../std/io/enum.ErrorKind.html#variant.UnexpectedEof
    #[stable(feature = "rw_exact_all_at", since = "1.33.0")]
    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read_at(buf, offset) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    offset += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(io::Error::new(io::ErrorKind::UnexpectedEof, "failed to fill whole buffer"))
        } else {
            Ok(())
        }
    }

    /// Writes a number of bytes starting from a given offset.
    ///
    /// Returns the number of bytes written.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// When writing beyond the end of the file, the file is appropriately
    /// extended and the intermediate bytes are initialized with the value 0.
    ///
    /// Note that similar to [`File::write`], it is not an error to return a
    /// short write.
    ///
    /// [`File::write`]: ../../../../std/fs/struct.File.html#write.v
    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        let bufs = &[IoSlice::new(buf)];
        self.write_vectored_at(bufs, offset)
    }

    /// Writes a number of bytes starting from a given offset.
    ///
    /// Returns the number of bytes written.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// When writing beyond the end of the file, the file is appropriately
    /// extended and the intermediate bytes are initialized with the value 0.
    ///
    /// Note that similar to [`File::write_vectored`], it is not an error to return a
    /// short write.
    ///
    /// [`File::write`]: ../../../../std/fs/struct.File.html#method.write_vectored
    fn write_vectored_at(&self, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize>;

    /// Attempts to write an entire buffer starting from a given offset.
    ///
    /// The offset is relative to the start of the file and thus independent
    /// from the current cursor.
    ///
    /// The current file cursor is not affected by this function.
    ///
    /// This method will continuously call [`write_at`] until there is no more data
    /// to be written or an error of non-[`ErrorKind::Interrupted`] kind is
    /// returned. This method will not return until the entire buffer has been
    /// successfully written or such an error occurs. The first error that is
    /// not of [`ErrorKind::Interrupted`] kind generated from this method will be
    /// returned.
    ///
    /// # Errors
    ///
    /// This function will return the first error of
    /// non-[`ErrorKind::Interrupted`] kind that [`write_at`] returns.
    ///
    /// [`ErrorKind::Interrupted`]: ../../../../std/io/enum.ErrorKind.html#variant.Interrupted
    /// [`write_at`]: #tymethod.write_at
    #[stable(feature = "rw_exact_all_at", since = "1.33.0")]
    fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.write_at(buf, offset) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ));
                }
                Ok(n) => {
                    buf = &buf[n..];
                    offset += n as u64
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Returns the current position within the file.
    ///
    /// This corresponds to the `fd_tell` syscall and is similar to
    /// `seek` where you offset 0 bytes from the current position.
    fn tell(&self) -> io::Result<u64>;

    /// Adjust the flags associated with this file.
    ///
    /// This corresponds to the `fd_fdstat_set_flags` syscall.
    fn fdstat_set_flags(&self, flags: u16) -> io::Result<()>;

    /// Adjust the rights associated with this file.
    ///
    /// This corresponds to the `fd_fdstat_set_rights` syscall.
    fn fdstat_set_rights(&self, rights: u64, inheriting: u64) -> io::Result<()>;

    /// Provide file advisory information on a file descriptor.
    ///
    /// This corresponds to the `fd_advise` syscall.
    fn advise(&self, offset: u64, len: u64, advice: u8) -> io::Result<()>;

    /// Force the allocation of space in a file.
    ///
    /// This corresponds to the `fd_allocate` syscall.
    fn allocate(&self, offset: u64, len: u64) -> io::Result<()>;

    /// Create a directory.
    ///
    /// This corresponds to the `path_create_directory` syscall.
    fn create_directory<P: AsRef<Path>>(&self, dir: P) -> io::Result<()>;

    /// Read the contents of a symbolic link.
    ///
    /// This corresponds to the `path_readlink` syscall.
    fn read_link<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf>;

    /// Return the attributes of a file or directory.
    ///
    /// This corresponds to the `path_filestat_get` syscall.
    fn metadata_at<P: AsRef<Path>>(&self, lookup_flags: u32, path: P) -> io::Result<Metadata>;

    /// Unlink a file.
    ///
    /// This corresponds to the `path_unlink_file` syscall.
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()>;

    /// Remove a directory.
    ///
    /// This corresponds to the `path_remove_directory` syscall.
    fn remove_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<()>;
}

// FIXME: bind fd_fdstat_get - need to define a custom return type
// FIXME: bind fd_readdir - can't return `ReadDir` since we only have entry name
// FIXME: bind fd_filestat_set_times maybe? - on crates.io for unix
// FIXME: bind path_filestat_set_times maybe? - on crates.io for unix
// FIXME: bind poll_oneoff maybe? - probably should wait for I/O to settle
// FIXME: bind random_get maybe? - on crates.io for unix

impl FileExt for fs::File {
    fn read_vectored_at(&self, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().fd().pread(bufs, offset)
    }

    fn write_vectored_at(&self, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().fd().pwrite(bufs, offset)
    }

    fn tell(&self) -> io::Result<u64> {
        self.as_inner().fd().tell()
    }

    fn fdstat_set_flags(&self, flags: u16) -> io::Result<()> {
        self.as_inner().fd().set_flags(flags)
    }

    fn fdstat_set_rights(&self, rights: u64, inheriting: u64) -> io::Result<()> {
        self.as_inner().fd().set_rights(rights, inheriting)
    }

    fn advise(&self, offset: u64, len: u64, advice: u8) -> io::Result<()> {
        self.as_inner().fd().advise(offset, len, advice)
    }

    fn allocate(&self, offset: u64, len: u64) -> io::Result<()> {
        self.as_inner().fd().allocate(offset, len)
    }

    fn create_directory<P: AsRef<Path>>(&self, dir: P) -> io::Result<()> {
        self.as_inner().fd().create_directory(osstr2str(dir.as_ref().as_ref())?)
    }

    fn read_link<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        self.as_inner().read_link(path.as_ref())
    }

    fn metadata_at<P: AsRef<Path>>(&self, lookup_flags: u32, path: P) -> io::Result<Metadata> {
        let m = self.as_inner().metadata_at(lookup_flags, path.as_ref())?;
        Ok(FromInner::from_inner(m))
    }

    fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.as_inner().fd().unlink_file(osstr2str(path.as_ref().as_ref())?)
    }

    fn remove_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.as_inner().fd().remove_directory(osstr2str(path.as_ref().as_ref())?)
    }
}

/// WASI-specific extensions to [`fs::OpenOptions`].
///
/// [`fs::OpenOptions`]: ../../../../std/fs/struct.OpenOptions.html
pub trait OpenOptionsExt {
    /// Pass custom `dirflags` argument to `path_open`.
    ///
    /// This option configures the `dirflags` argument to the
    /// `path_open` syscall which `OpenOptions` will eventually call. The
    /// `dirflags` argument configures how the file is looked up, currently
    /// primarily affecting whether symlinks are followed or not.
    ///
    /// By default this value is `__WASI_LOOKUP_SYMLINK_FOLLOW`, or symlinks are
    /// followed. You can call this method with 0 to disable following symlinks
    fn lookup_flags(&mut self, flags: u32) -> &mut Self;

    /// Indicates whether `OpenOptions` must open a directory or not.
    ///
    /// This method will configure whether the `__WASI_O_DIRECTORY` flag is
    /// passed when opening a file. When passed it will require that the opened
    /// path is a directory.
    ///
    /// This option is by default `false`
    fn directory(&mut self, dir: bool) -> &mut Self;

    /// Indicates whether `__WASI_FDFLAG_DSYNC` is passed in the `fs_flags`
    /// field of `path_open`.
    ///
    /// This option is by default `false`
    fn dsync(&mut self, dsync: bool) -> &mut Self;

    /// Indicates whether `__WASI_FDFLAG_NONBLOCK` is passed in the `fs_flags`
    /// field of `path_open`.
    ///
    /// This option is by default `false`
    fn nonblock(&mut self, nonblock: bool) -> &mut Self;

    /// Indicates whether `__WASI_FDFLAG_RSYNC` is passed in the `fs_flags`
    /// field of `path_open`.
    ///
    /// This option is by default `false`
    fn rsync(&mut self, rsync: bool) -> &mut Self;

    /// Indicates whether `__WASI_FDFLAG_SYNC` is passed in the `fs_flags`
    /// field of `path_open`.
    ///
    /// This option is by default `false`
    fn sync(&mut self, sync: bool) -> &mut Self;

    /// Indicates the value that should be passed in for the `fs_rights_base`
    /// parameter of `path_open`.
    ///
    /// This option defaults based on the `read` and `write` configuration of
    /// this `OpenOptions` builder. If this method is called, however, the
    /// exact mask passed in will be used instead.
    fn fs_rights_base(&mut self, rights: u64) -> &mut Self;

    /// Indicates the value that should be passed in for the
    /// `fs_rights_inheriting` parameter of `path_open`.
    ///
    /// The default for this option is the same value as what will be passed
    /// for the `fs_rights_base` parameter but if this method is called then
    /// the specified value will be used instead.
    fn fs_rights_inheriting(&mut self, rights: u64) -> &mut Self;

    /// Open a file or directory.
    ///
    /// This corresponds to the `path_open` syscall.
    fn open_at<P: AsRef<Path>>(&self, file: &File, path: P) -> io::Result<File>;
}

impl OpenOptionsExt for OpenOptions {
    fn lookup_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.as_inner_mut().lookup_flags(flags);
        self
    }

    fn directory(&mut self, dir: bool) -> &mut OpenOptions {
        self.as_inner_mut().directory(dir);
        self
    }

    fn dsync(&mut self, enabled: bool) -> &mut OpenOptions {
        self.as_inner_mut().dsync(enabled);
        self
    }

    fn nonblock(&mut self, enabled: bool) -> &mut OpenOptions {
        self.as_inner_mut().nonblock(enabled);
        self
    }

    fn rsync(&mut self, enabled: bool) -> &mut OpenOptions {
        self.as_inner_mut().rsync(enabled);
        self
    }

    fn sync(&mut self, enabled: bool) -> &mut OpenOptions {
        self.as_inner_mut().sync(enabled);
        self
    }

    fn fs_rights_base(&mut self, rights: u64) -> &mut OpenOptions {
        self.as_inner_mut().fs_rights_base(rights);
        self
    }

    fn fs_rights_inheriting(&mut self, rights: u64) -> &mut OpenOptions {
        self.as_inner_mut().fs_rights_inheriting(rights);
        self
    }

    fn open_at<P: AsRef<Path>>(&self, file: &File, path: P) -> io::Result<File> {
        let inner = file.as_inner().open_at(path.as_ref(), self.as_inner())?;
        Ok(File::from_inner(inner))
    }
}

/// WASI-specific extensions to [`fs::Metadata`].
///
/// [`fs::Metadata`]: ../../../../std/fs/struct.Metadata.html
pub trait MetadataExt {
    /// Returns the `st_dev` field of the internal `filestat_t`
    fn dev(&self) -> u64;
    /// Returns the `st_ino` field of the internal `filestat_t`
    fn ino(&self) -> u64;
    /// Returns the `st_nlink` field of the internal `filestat_t`
    fn nlink(&self) -> u64;
    /// Returns the `st_atim` field of the internal `filestat_t`
    fn atim(&self) -> u64;
    /// Returns the `st_mtim` field of the internal `filestat_t`
    fn mtim(&self) -> u64;
    /// Returns the `st_ctim` field of the internal `filestat_t`
    fn ctim(&self) -> u64;
}

impl MetadataExt for fs::Metadata {
    fn dev(&self) -> u64 {
        self.as_inner().as_wasi().dev
    }
    fn ino(&self) -> u64 {
        self.as_inner().as_wasi().ino
    }
    fn nlink(&self) -> u64 {
        self.as_inner().as_wasi().nlink
    }
    fn atim(&self) -> u64 {
        self.as_inner().as_wasi().atim
    }
    fn mtim(&self) -> u64 {
        self.as_inner().as_wasi().mtim
    }
    fn ctim(&self) -> u64 {
        self.as_inner().as_wasi().ctim
    }
}

/// WASI-specific extensions for [`FileType`].
///
/// Adds support for special WASI file types such as block/character devices,
/// pipes, and sockets.
///
/// [`FileType`]: ../../../../std/fs/struct.FileType.html
pub trait FileTypeExt {
    /// Returns `true` if this file type is a block device.
    fn is_block_device(&self) -> bool;
    /// Returns `true` if this file type is a character device.
    fn is_character_device(&self) -> bool;
    /// Returns `true` if this file type is a socket datagram.
    fn is_socket_dgram(&self) -> bool;
    /// Returns `true` if this file type is a socket stream.
    fn is_socket_stream(&self) -> bool;
}

impl FileTypeExt for fs::FileType {
    fn is_block_device(&self) -> bool {
        self.as_inner().bits() == wasi::FILETYPE_BLOCK_DEVICE
    }
    fn is_character_device(&self) -> bool {
        self.as_inner().bits() == wasi::FILETYPE_CHARACTER_DEVICE
    }
    fn is_socket_dgram(&self) -> bool {
        self.as_inner().bits() == wasi::FILETYPE_SOCKET_DGRAM
    }
    fn is_socket_stream(&self) -> bool {
        self.as_inner().bits() == wasi::FILETYPE_SOCKET_STREAM
    }
}

/// WASI-specific extension methods for [`fs::DirEntry`].
///
/// [`fs::DirEntry`]: ../../../../std/fs/struct.DirEntry.html
pub trait DirEntryExt {
    /// Returns the underlying `d_ino` field of the `dirent_t`
    fn ino(&self) -> u64;
}

impl DirEntryExt for fs::DirEntry {
    fn ino(&self) -> u64 {
        self.as_inner().ino()
    }
}

/// Create a hard link.
///
/// This corresponds to the `path_link` syscall.
pub fn link<P: AsRef<Path>, U: AsRef<Path>>(
    old_fd: &File,
    old_flags: u32,
    old_path: P,
    new_fd: &File,
    new_path: U,
) -> io::Result<()> {
    old_fd.as_inner().fd().link(
        old_flags,
        osstr2str(old_path.as_ref().as_ref())?,
        new_fd.as_inner().fd(),
        osstr2str(new_path.as_ref().as_ref())?,
    )
}

/// Rename a file or directory.
///
/// This corresponds to the `path_rename` syscall.
pub fn rename<P: AsRef<Path>, U: AsRef<Path>>(
    old_fd: &File,
    old_path: P,
    new_fd: &File,
    new_path: U,
) -> io::Result<()> {
    old_fd.as_inner().fd().rename(
        osstr2str(old_path.as_ref().as_ref())?,
        new_fd.as_inner().fd(),
        osstr2str(new_path.as_ref().as_ref())?,
    )
}

/// Create a symbolic link.
///
/// This corresponds to the `path_symlink` syscall.
pub fn symlink<P: AsRef<Path>, U: AsRef<Path>>(
    old_path: P,
    fd: &File,
    new_path: U,
) -> io::Result<()> {
    fd.as_inner()
        .fd()
        .symlink(osstr2str(old_path.as_ref().as_ref())?, osstr2str(new_path.as_ref().as_ref())?)
}
