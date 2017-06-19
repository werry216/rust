// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Inspection and manipulation of the process's environment.
//!
//! This module contains functions to inspect various aspects such as
//! environment variables, process arguments, the current directory, and various
//! other important directories.
//!
//! There are several functions and structs in this module that have a
//! counterpart ending in `os`. Those ending in `os` will return an [`OsString`]
//! and those without will be returning a [`String`].
//!
//! [`OsString`]: ../../std/ffi/struct.OsString.html
//! [`String`]: ../string/struct.String.html

#![stable(feature = "env", since = "1.0.0")]

use error::Error;
use ffi::{OsStr, OsString};
use fmt;
use io;
use path::{Path, PathBuf};
use sys;
use sys::os as os_imp;

/// Returns the current working directory as a [`PathBuf`].
///
/// # Errors
///
/// Returns an [`Err`] if the current working directory value is invalid.
/// Possible cases:
///
/// * Current directory does not exist.
/// * There are insufficient permissions to access the current directory.
///
/// [`PathBuf`]: ../../std/path/struct.PathBuf.html
/// [`Err`]: ../../std/result/enum.Result.html#method.err
///
/// # Examples
///
/// ```
/// use std::env;
///
/// // We assume that we are in a valid directory.
/// let path = env::current_dir().unwrap();
/// println!("The current directory is {}", path.display());
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn current_dir() -> io::Result<PathBuf> {
    os_imp::getcwd()
}

/// Changes the current working directory to the specified path.
///
/// Returns an [`Err`] if the operation fails.
///
/// [`Err`]: ../../std/result/enum.Result.html#method.err
///
/// # Examples
///
/// ```
/// use std::env;
/// use std::path::Path;
///
/// let root = Path::new("/");
/// assert!(env::set_current_dir(&root).is_ok());
/// println!("Successfully changed working directory to {}!", root.display());
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn set_current_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    os_imp::chdir(path.as_ref())
}

/// An iterator over a snapshot of the environment variables of this process.
///
/// This structure is created by the [`std::env::vars`] function. See its
/// documentation for more.
///
/// [`std::env::vars`]: fn.vars.html
#[stable(feature = "env", since = "1.0.0")]
pub struct Vars { inner: VarsOs }

/// An iterator over a snapshot of the environment variables of this process.
///
/// This structure is created by the [`std::env::vars_os`] function. See
/// its documentation for more.
///
/// [`std::env::vars_os`]: fn.vars_os.html
#[stable(feature = "env", since = "1.0.0")]
pub struct VarsOs { inner: os_imp::Env }

/// Returns an iterator of (variable, value) pairs of strings, for all the
/// environment variables of the current process.
///
/// The returned iterator contains a snapshot of the process's environment
/// variables at the time of this invocation. Modifications to environment
/// variables afterwards will not be reflected in the returned iterator.
///
/// # Panics
///
/// While iterating, the returned iterator will panic if any key or value in the
/// environment is not valid unicode. If this is not desired, consider using the
/// [`env::vars_os`] function.
///
/// [`env::vars_os`]: fn.vars_os.html
///
/// # Examples
///
/// ```
/// use std::env;
///
/// // We will iterate through the references to the element returned by
/// // env::vars();
/// for (key, value) in env::vars() {
///     println!("{}: {}", key, value);
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn vars() -> Vars {
    Vars { inner: vars_os() }
}

/// Returns an iterator of (variable, value) pairs of OS strings, for all the
/// environment variables of the current process.
///
/// The returned iterator contains a snapshot of the process's environment
/// variables at the time of this invocation. Modifications to environment
/// variables afterwards will not be reflected in the returned iterator.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// // We will iterate through the references to the element returned by
/// // env::vars_os();
/// for (key, value) in env::vars_os() {
///     println!("{:?}: {:?}", key, value);
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn vars_os() -> VarsOs {
    VarsOs { inner: os_imp::env() }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for Vars {
    type Item = (String, String);
    fn next(&mut self) -> Option<(String, String)> {
        self.inner.next().map(|(a, b)| {
            (a.into_string().unwrap(), b.into_string().unwrap())
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Vars {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Vars { .. }")
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for VarsOs {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> { self.inner.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for VarsOs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("VarsOs { .. }")
    }
}

/// Fetches the environment variable `key` from the current process.
///
/// # Errors
///
/// * Environment variable is not present
/// * Environment variable is not valid unicode
///
/// # Examples
///
/// ```
/// use std::env;
///
/// let key = "HOME";
/// match env::var(key) {
///     Ok(val) => println!("{}: {:?}", key, val),
///     Err(e) => println!("couldn't interpret {}: {}", key, e),
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn var<K: AsRef<OsStr>>(key: K) -> Result<String, VarError> {
    _var(key.as_ref())
}

fn _var(key: &OsStr) -> Result<String, VarError> {
    match var_os(key) {
        Some(s) => s.into_string().map_err(VarError::NotUnicode),
        None => Err(VarError::NotPresent),
    }
}

/// Fetches the environment variable `key` from the current process, returning
/// [`None`] if the variable isn't set.
///
/// [`None`]: ../option/enum.Option.html#variant.None
///
/// # Examples
///
/// ```
/// use std::env;
///
/// let key = "HOME";
/// match env::var_os(key) {
///     Some(val) => println!("{}: {:?}", key, val),
///     None => println!("{} is not defined in the environment.", key)
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn var_os<K: AsRef<OsStr>>(key: K) -> Option<OsString> {
    _var_os(key.as_ref())
}

fn _var_os(key: &OsStr) -> Option<OsString> {
    os_imp::getenv(key).unwrap_or_else(|e| {
        panic!("failed to get environment variable `{:?}`: {}", key, e)
    })
}

/// The error type for operations interacting with environment variables.
/// Possibly returned from the [`env::var`] function.
///
/// [`env::var`]: fn.var.html
#[derive(Debug, PartialEq, Eq, Clone)]
#[stable(feature = "env", since = "1.0.0")]
pub enum VarError {
    /// The specified environment variable was not present in the current
    /// process's environment.
    #[stable(feature = "env", since = "1.0.0")]
    NotPresent,

    /// The specified environment variable was found, but it did not contain
    /// valid unicode data. The found data is returned as a payload of this
    /// variant.
    #[stable(feature = "env", since = "1.0.0")]
    NotUnicode(#[stable(feature = "env", since = "1.0.0")] OsString),
}

#[stable(feature = "env", since = "1.0.0")]
impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            VarError::NotPresent => write!(f, "environment variable not found"),
            VarError::NotUnicode(ref s) => {
                write!(f, "environment variable was not valid unicode: {:?}", s)
            }
        }
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Error for VarError {
    fn description(&self) -> &str {
        match *self {
            VarError::NotPresent => "environment variable not found",
            VarError::NotUnicode(..) => "environment variable was not valid unicode",
        }
    }
}

/// Sets the environment variable `k` to the value `v` for the currently running
/// process.
///
/// Note that while concurrent access to environment variables is safe in Rust,
/// some platforms only expose inherently unsafe non-threadsafe APIs for
/// inspecting the environment. As a result extra care needs to be taken when
/// auditing calls to unsafe external FFI functions to ensure that any external
/// environment accesses are properly synchronized with accesses in Rust.
///
/// Discussion of this unsafety on Unix may be found in:
///
///  - [Austin Group Bugzilla](http://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// # Panics
///
/// This function may panic if `key` is empty, contains an ASCII equals sign
/// `'='` or the NUL character `'\0'`, or when the value contains the NUL
/// character.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// let key = "KEY";
/// env::set_var(key, "VALUE");
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn set_var<K: AsRef<OsStr>, V: AsRef<OsStr>>(k: K, v: V) {
    _set_var(k.as_ref(), v.as_ref())
}

fn _set_var(k: &OsStr, v: &OsStr) {
    os_imp::setenv(k, v).unwrap_or_else(|e| {
        panic!("failed to set environment variable `{:?}` to `{:?}`: {}",
               k, v, e)
    })
}

/// Removes an environment variable from the environment of the currently running process.
///
/// Note that while concurrent access to environment variables is safe in Rust,
/// some platforms only expose inherently unsafe non-threadsafe APIs for
/// inspecting the environment. As a result extra care needs to be taken when
/// auditing calls to unsafe external FFI functions to ensure that any external
/// environment accesses are properly synchronized with accesses in Rust.
///
/// Discussion of this unsafety on Unix may be found in:
///
///  - [Austin Group Bugzilla](http://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// # Panics
///
/// This function may panic if `key` is empty, contains an ASCII equals sign
/// `'='` or the NUL character `'\0'`, or when the value contains the NUL
/// character.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// let key = "KEY";
/// env::set_var(key, "VALUE");
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
///
/// env::remove_var(key);
/// assert!(env::var(key).is_err());
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn remove_var<K: AsRef<OsStr>>(k: K) {
    _remove_var(k.as_ref())
}

fn _remove_var(k: &OsStr) {
    os_imp::unsetenv(k).unwrap_or_else(|e| {
        panic!("failed to remove environment variable `{:?}`: {}", k, e)
    })
}

/// An iterator that splits an environment variable into paths according to
/// platform-specific conventions.
///
/// This structure is created by the [`std::env::split_paths`] function. See its
/// documentation for more.
///
/// [`std::env::split_paths`]: fn.split_paths.html
#[stable(feature = "env", since = "1.0.0")]
pub struct SplitPaths<'a> { inner: os_imp::SplitPaths<'a> }

/// Parses input according to platform conventions for the `PATH`
/// environment variable.
///
/// Returns an iterator over the paths contained in `unparsed`.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// let key = "PATH";
/// match env::var_os(key) {
///     Some(paths) => {
///         for path in env::split_paths(&paths) {
///             println!("'{}'", path.display());
///         }
///     }
///     None => println!("{} is not defined in the environment.", key)
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn split_paths<T: AsRef<OsStr> + ?Sized>(unparsed: &T) -> SplitPaths {
    SplitPaths { inner: os_imp::split_paths(unparsed.as_ref()) }
}

#[stable(feature = "env", since = "1.0.0")]
impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> { self.inner.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<'a> fmt::Debug for SplitPaths<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("SplitPaths { .. }")
    }
}

/// The error type for operations on the `PATH` variable. Possibly returned from
/// the [`env::join_paths`] function.
///
/// [`env::join_paths`]: fn.join_paths.html
#[derive(Debug)]
#[stable(feature = "env", since = "1.0.0")]
pub struct JoinPathsError {
    inner: os_imp::JoinPathsError
}

/// Joins a collection of [`Path`]s appropriately for the `PATH`
/// environment variable.
///
/// # Errors
///
/// Returns an [`Err`][err] (containing an error message) if one of the input
/// [`Path`]s contains an invalid character for constructing the `PATH`
/// variable (a double quote on Windows or a colon on Unix).
///
/// [`Path`]: ../../std/path/struct.Path.html
/// [`OsString`]: ../../std/ffi/struct.OsString.html
/// [err]: ../../std/result/enum.Result.html#variant.Err
///
/// # Examples
///
/// ```
/// use std::env;
/// use std::path::PathBuf;
///
/// if let Some(path) = env::var_os("PATH") {
///     let mut paths = env::split_paths(&path).collect::<Vec<_>>();
///     paths.push(PathBuf::from("/home/xyz/bin"));
///     let new_path = env::join_paths(paths).unwrap();
///     env::set_var("PATH", &new_path);
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
    where I: IntoIterator<Item=T>, T: AsRef<OsStr>
{
    os_imp::join_paths(paths.into_iter()).map_err(|e| {
        JoinPathsError { inner: e }
    })
}

#[stable(feature = "env", since = "1.0.0")]
impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Error for JoinPathsError {
    fn description(&self) -> &str { self.inner.description() }
}

/// Returns the path of the current user's home directory if known.
///
/// # Unix
///
/// Returns the value of the 'HOME' environment variable if it is set
/// and not equal to the empty string. Otherwise, it tries to determine the
/// home directory by invoking the `getpwuid_r` function on the UID of the
/// current user.
///
/// # Windows
///
/// Returns the value of the 'HOME' environment variable if it is
/// set and not equal to the empty string. Otherwise, returns the value of the
/// 'USERPROFILE' environment variable if it is set and not equal to the empty
/// string. If both do not exist, [`GetUserProfileDirectory`][msdn] is used to
/// return the appropriate path.
///
/// [msdn]: https://msdn.microsoft.com/en-us/library/windows/desktop/bb762280(v=vs.85).aspx
///
/// # Examples
///
/// ```
/// use std::env;
///
/// match env::home_dir() {
///     Some(path) => println!("{}", path.display()),
///     None => println!("Impossible to get your home dir!"),
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn home_dir() -> Option<PathBuf> {
    os_imp::home_dir()
}

/// Returns the path of a temporary directory.
///
/// # Unix
///
/// Returns the value of the `TMPDIR` environment variable if it is
/// set, otherwise for non-Android it returns `/tmp`. If Android, since there
/// is no global temporary folder (it is usually allocated per-app), it returns
/// `/data/local/tmp`.
///
/// # Windows
///
/// Returns the value of, in order, the `TMP`, `TEMP`,
/// `USERPROFILE` environment variable if any are set and not the empty
/// string. Otherwise, `temp_dir` returns the path of the Windows directory.
/// This behavior is identical to that of [`GetTempPath`][msdn], which this
/// function uses internally.
///
/// [msdn]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa364992(v=vs.85).aspx
///
/// ```
/// use std::env;
/// use std::fs::File;
///
/// # fn foo() -> std::io::Result<()> {
/// let mut dir = env::temp_dir();
/// dir.push("foo.txt");
///
/// let f = File::create(dir)?;
/// # Ok(())
/// # }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn temp_dir() -> PathBuf {
    os_imp::temp_dir()
}

/// Returns the full filesystem path of the current running executable.
///
/// The path returned is not necessarily a "real path" of the executable as
/// there may be intermediate symlinks.
///
/// # Errors
///
/// Acquiring the path of the current executable is a platform-specific operation
/// that can fail for a good number of reasons. Some errors can include, but not
/// be limited to, filesystem operations failing or general syscall failures.
///
/// # Security
///
/// The output of this function should not be used in anything that might have
/// security implications. For example:
///
/// ```
/// fn main() {
///     println!("{:?}", std::env::current_exe());
/// }
/// ```
///
/// On Linux systems, if this is compiled as `foo`:
///
/// ```bash
/// $ rustc foo.rs
/// $ ./foo
/// Ok("/home/alex/foo")
/// ```
///
/// And you make a symbolic link of the program:
///
/// ```bash
/// $ ln foo bar
/// ```
///
/// When you run it, you won't get the original executable, you'll get the
/// symlink:
///
/// ```bash
/// $ ./bar
/// Ok("/home/alex/bar")
/// ```
///
/// This sort of behavior has been known to [lead to privilege escalation] when
/// used incorrectly, for example.
///
/// [lead to privilege escalation]: http://securityvulns.com/Wdocument183.html
///
/// # Examples
///
/// ```
/// use std::env;
///
/// match env::current_exe() {
///     Ok(exe_path) => println!("Path of this executable is: {}",
///                               exe_path.display()),
///     Err(e) => println!("failed to get current exe path: {}", e),
/// };
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn current_exe() -> io::Result<PathBuf> {
    os_imp::current_exe()
}

/// An iterator over the arguments of a process, yielding a [`String`] value for
/// each argument.
///
/// This struct is created by the [`std::env::args`] function. See its
/// documentation for more.
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and may not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// [`String`]: ../string/struct.String.html
/// [`std::env::args`]: ./fn.args.html
#[stable(feature = "env", since = "1.0.0")]
pub struct Args { inner: ArgsOs }

/// An iterator over the arguments of a process, yielding an [`OsString`] value
/// for each argument.
///
/// This struct is created by the [`std::env::args_os`] function. See its
/// documentation for more.
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and may not even exist. This means this property
/// should not be relied upon for security purposes.
///
/// [`OsString`]: ../ffi/struct.OsString.html
/// [`std::env::args_os`]: ./fn.args_os.html
#[stable(feature = "env", since = "1.0.0")]
pub struct ArgsOs { inner: sys::args::Args }

/// Returns the arguments which this program was started with (normally passed
/// via the command line).
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and may not even exist. This means this property should
/// not be relied upon for security purposes.
///
/// # Panics
///
/// The returned iterator will panic during iteration if any argument to the
/// process is not valid unicode. If this is not desired,
/// use the [`args_os`] function instead.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// // Prints each argument on a separate line
/// for argument in env::args() {
///     println!("{}", argument);
/// }
/// ```
///
/// [`args_os`]: ./fn.args_os.html
#[stable(feature = "env", since = "1.0.0")]
pub fn args() -> Args {
    Args { inner: args_os() }
}

/// Returns the arguments which this program was started with (normally passed
/// via the command line).
///
/// The first element is traditionally the path of the executable, but it can be
/// set to arbitrary text, and it may not even exist, so this property should
/// not be relied upon for security purposes.
///
/// # Examples
///
/// ```
/// use std::env;
///
/// // Prints each argument on a separate line
/// for argument in env::args_os() {
///     println!("{:?}", argument);
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn args_os() -> ArgsOs {
    ArgsOs { inner: sys::args::args() }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for Args {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        self.inner.next().map(|s| s.into_string().unwrap())
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}

#[stable(feature = "env", since = "1.0.0")]
impl ExactSizeIterator for Args {
    fn len(&self) -> usize { self.inner.len() }
    fn is_empty(&self) -> bool { self.inner.is_empty() }
}

#[stable(feature = "env_iterators", since = "1.12.0")]
impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<String> {
        self.inner.next_back().map(|s| s.into_string().unwrap())
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("Args { .. }")
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for ArgsOs {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> { self.inner.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }
}

#[stable(feature = "env", since = "1.0.0")]
impl ExactSizeIterator for ArgsOs {
    fn len(&self) -> usize { self.inner.len() }
    fn is_empty(&self) -> bool { self.inner.is_empty() }
}

#[stable(feature = "env_iterators", since = "1.12.0")]
impl DoubleEndedIterator for ArgsOs {
    fn next_back(&mut self) -> Option<OsString> { self.inner.next_back() }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for ArgsOs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad("ArgsOs { .. }")
    }
}

/// Constants associated with the current target
#[stable(feature = "env", since = "1.0.0")]
pub mod consts {
    use sys::env::os;

    /// A string describing the architecture of the CPU that is currently
    /// in use.
    ///
    /// Some possible values:
    ///
    /// - x86
    /// - x86_64
    /// - arm
    /// - aarch64
    /// - mips
    /// - mips64
    /// - powerpc
    /// - powerpc64
    /// - s390x
    /// - sparc64
    #[stable(feature = "env", since = "1.0.0")]
    pub const ARCH: &'static str = super::arch::ARCH;

    /// The family of the operating system. Example value is `unix`.
    ///
    /// Some possible values:
    ///
    /// - unix
    /// - windows
    #[stable(feature = "env", since = "1.0.0")]
    pub const FAMILY: &'static str = os::FAMILY;

    /// A string describing the specific operating system in use.
    /// Example value is `linux`.
    ///
    /// Some possible values:
    ///
    /// - linux
    /// - macos
    /// - ios
    /// - freebsd
    /// - dragonfly
    /// - bitrig
    /// - netbsd
    /// - openbsd
    /// - solaris
    /// - android
    /// - windows
    #[stable(feature = "env", since = "1.0.0")]
    pub const OS: &'static str = os::OS;

    /// Specifies the filename prefix used for shared libraries on this
    /// platform. Example value is `lib`.
    ///
    /// Some possible values:
    ///
    /// - lib
    /// - `""` (an empty string)
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_PREFIX: &'static str = os::DLL_PREFIX;

    /// Specifies the filename suffix used for shared libraries on this
    /// platform. Example value is `.so`.
    ///
    /// Some possible values:
    ///
    /// - .so
    /// - .dylib
    /// - .dll
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_SUFFIX: &'static str = os::DLL_SUFFIX;

    /// Specifies the file extension used for shared libraries on this
    /// platform that goes after the dot. Example value is `so`.
    ///
    /// Some possible values:
    ///
    /// - so
    /// - dylib
    /// - dll
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_EXTENSION: &'static str = os::DLL_EXTENSION;

    /// Specifies the filename suffix used for executable binaries on this
    /// platform. Example value is `.exe`.
    ///
    /// Some possible values:
    ///
    /// - .exe
    /// - .nexe
    /// - .pexe
    /// - `""` (an empty string)
    #[stable(feature = "env", since = "1.0.0")]
    pub const EXE_SUFFIX: &'static str = os::EXE_SUFFIX;

    /// Specifies the file extension, if any, used for executable binaries
    /// on this platform. Example value is `exe`.
    ///
    /// Some possible values:
    ///
    /// - exe
    /// - `""` (an empty string)
    #[stable(feature = "env", since = "1.0.0")]
    pub const EXE_EXTENSION: &'static str = os::EXE_EXTENSION;
}

#[cfg(target_arch = "x86")]
mod arch {
    pub const ARCH: &'static str = "x86";
}

#[cfg(target_arch = "x86_64")]
mod arch {
    pub const ARCH: &'static str = "x86_64";
}

#[cfg(target_arch = "arm")]
mod arch {
    pub const ARCH: &'static str = "arm";
}

#[cfg(target_arch = "aarch64")]
mod arch {
    pub const ARCH: &'static str = "aarch64";
}

#[cfg(target_arch = "mips")]
mod arch {
    pub const ARCH: &'static str = "mips";
}

#[cfg(target_arch = "mips64")]
mod arch {
    pub const ARCH: &'static str = "mips64";
}

#[cfg(target_arch = "powerpc")]
mod arch {
    pub const ARCH: &'static str = "powerpc";
}

#[cfg(target_arch = "powerpc64")]
mod arch {
    pub const ARCH: &'static str = "powerpc64";
}

#[cfg(target_arch = "s390x")]
mod arch {
    pub const ARCH: &'static str = "s390x";
}

#[cfg(target_arch = "sparc64")]
mod arch {
    pub const ARCH: &'static str = "sparc64";
}

#[cfg(target_arch = "le32")]
mod arch {
    pub const ARCH: &'static str = "le32";
}

#[cfg(target_arch = "asmjs")]
mod arch {
    pub const ARCH: &'static str = "asmjs";
}

#[cfg(target_arch = "wasm32")]
mod arch {
    pub const ARCH: &'static str = "wasm32";
}

#[cfg(test)]
mod tests {
    use super::*;

    use iter::repeat;
    use rand::{self, Rng};
    use ffi::{OsString, OsStr};
    use path::{Path, PathBuf};

    fn make_rand_name() -> OsString {
        let mut rng = rand::thread_rng();
        let n = format!("TEST{}", rng.gen_ascii_chars().take(10)
                                     .collect::<String>());
        let n = OsString::from(n);
        assert!(var_os(&n).is_none());
        n
    }

    fn eq(a: Option<OsString>, b: Option<&str>) {
        assert_eq!(a.as_ref().map(|s| &**s), b.map(OsStr::new).map(|s| &*s));
    }

    #[test]
    fn test_set_var() {
        let n = make_rand_name();
        set_var(&n, "VALUE");
        eq(var_os(&n), Some("VALUE"));
    }

    #[test]
    fn test_remove_var() {
        let n = make_rand_name();
        set_var(&n, "VALUE");
        remove_var(&n);
        eq(var_os(&n), None);
    }

    #[test]
    fn test_set_var_overwrite() {
        let n = make_rand_name();
        set_var(&n, "1");
        set_var(&n, "2");
        eq(var_os(&n), Some("2"));
        set_var(&n, "");
        eq(var_os(&n), Some(""));
    }

    #[test]
    #[cfg_attr(target_os = "emscripten", ignore)]
    fn test_var_big() {
        let mut s = "".to_string();
        let mut i = 0;
        while i < 100 {
            s.push_str("aaaaaaaaaa");
            i += 1;
        }
        let n = make_rand_name();
        set_var(&n, &s);
        eq(var_os(&n), Some(&s));
    }

    #[test]
    #[cfg_attr(target_os = "emscripten", ignore)]
    fn test_self_exe_path() {
        let path = current_exe();
        assert!(path.is_ok());
        let path = path.unwrap();

        // Hard to test this function
        assert!(path.is_absolute());
    }

    #[test]
    #[cfg_attr(target_os = "emscripten", ignore)]
    fn test_env_set_get_huge() {
        let n = make_rand_name();
        let s = repeat("x").take(10000).collect::<String>();
        set_var(&n, &s);
        eq(var_os(&n), Some(&s));
        remove_var(&n);
        eq(var_os(&n), None);
    }

    #[test]
    fn test_env_set_var() {
        let n = make_rand_name();

        let mut e = vars_os();
        set_var(&n, "VALUE");
        assert!(!e.any(|(k, v)| {
            &*k == &*n && &*v == "VALUE"
        }));

        assert!(vars_os().any(|(k, v)| {
            &*k == &*n && &*v == "VALUE"
        }));
    }

    #[test]
    fn test() {
        assert!((!Path::new("test-path").is_absolute()));

        current_dir().unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn split_paths_windows() {
        fn check_parse(unparsed: &str, parsed: &[&str]) -> bool {
            split_paths(unparsed).collect::<Vec<_>>() ==
                parsed.iter().map(|s| PathBuf::from(*s)).collect::<Vec<_>>()
        }

        assert!(check_parse("", &mut [""]));
        assert!(check_parse(r#""""#, &mut [""]));
        assert!(check_parse(";;", &mut ["", "", ""]));
        assert!(check_parse(r"c:\", &mut [r"c:\"]));
        assert!(check_parse(r"c:\;", &mut [r"c:\", ""]));
        assert!(check_parse(r"c:\;c:\Program Files\",
                            &mut [r"c:\", r"c:\Program Files\"]));
        assert!(check_parse(r#"c:\;c:\"foo"\"#, &mut [r"c:\", r"c:\foo\"]));
        assert!(check_parse(r#"c:\;c:\"foo;bar"\;c:\baz"#,
                            &mut [r"c:\", r"c:\foo;bar\", r"c:\baz"]));
    }

    #[test]
    #[cfg(unix)]
    fn split_paths_unix() {
        fn check_parse(unparsed: &str, parsed: &[&str]) -> bool {
            split_paths(unparsed).collect::<Vec<_>>() ==
                parsed.iter().map(|s| PathBuf::from(*s)).collect::<Vec<_>>()
        }

        assert!(check_parse("", &mut [""]));
        assert!(check_parse("::", &mut ["", "", ""]));
        assert!(check_parse("/", &mut ["/"]));
        assert!(check_parse("/:", &mut ["/", ""]));
        assert!(check_parse("/:/usr/local", &mut ["/", "/usr/local"]));
    }

    #[test]
    #[cfg(unix)]
    fn join_paths_unix() {
        fn test_eq(input: &[&str], output: &str) -> bool {
            &*join_paths(input.iter().cloned()).unwrap() ==
                OsStr::new(output)
        }

        assert!(test_eq(&[], ""));
        assert!(test_eq(&["/bin", "/usr/bin", "/usr/local/bin"],
                         "/bin:/usr/bin:/usr/local/bin"));
        assert!(test_eq(&["", "/bin", "", "", "/usr/bin", ""],
                         ":/bin:::/usr/bin:"));
        assert!(join_paths(["/te:st"].iter().cloned()).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn join_paths_windows() {
        fn test_eq(input: &[&str], output: &str) -> bool {
            &*join_paths(input.iter().cloned()).unwrap() ==
                OsStr::new(output)
        }

        assert!(test_eq(&[], ""));
        assert!(test_eq(&[r"c:\windows", r"c:\"],
                        r"c:\windows;c:\"));
        assert!(test_eq(&["", r"c:\windows", "", "", r"c:\", ""],
                        r";c:\windows;;;c:\;"));
        assert!(test_eq(&[r"c:\te;st", r"c:\"],
                        r#""c:\te;st";c:\"#));
        assert!(join_paths([r#"c:\te"st"#].iter().cloned()).is_err());
    }
    }
