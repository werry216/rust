// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Panic support in the standard library.

#![unstable(feature = "core_panic_info",
            reason = "newly available in libcore",
            issue = "44489")]

use any::Any;

/// A struct providing information about a panic.
///
/// `PanicInfo` structure is passed to a panic hook set by the [`set_hook`]
/// function.
///
/// [`set_hook`]: ../../std/panic/fn.set_hook.html
///
/// # Examples
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|panic_info| {
///     println!("panic occurred: {:?}", panic_info.payload().downcast_ref::<&str>().unwrap());
/// }));
///
/// panic!("Normal panic");
/// ```
#[stable(feature = "panic_hooks", since = "1.10.0")]
#[derive(Debug)]
pub struct PanicInfo<'a> {
    payload: &'a (Any + Send),
    location: Location<'a>,
}

impl<'a> PanicInfo<'a> {
    #![unstable(feature = "panic_internals",
                reason = "internal details of the implementation of the `panic!` \
                          and related macros",
                issue = "0")]
    #[doc(hidden)]
    pub fn internal_constructor(payload: &'a (Any + Send), location: Location<'a>,) -> Self {
        PanicInfo { payload, location }
    }

    /// Returns the payload associated with the panic.
    ///
    /// This will commonly, but not always, be a `&'static str` or [`String`].
    ///
    /// [`String`]: ../../std/string/struct.String.html
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     println!("panic occurred: {:?}", panic_info.payload().downcast_ref::<&str>().unwrap());
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn payload(&self) -> &(Any + Send) {
        self.payload
    }

    /// Returns information about the location from which the panic originated,
    /// if available.
    ///
    /// This method will currently always return [`Some`], but this may change
    /// in future versions.
    ///
    /// [`Some`]: ../../std/option/enum.Option.html#variant.Some
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}' at line {}", location.file(),
    ///             location.line());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn location(&self) -> Option<&Location> {
        // NOTE: If this is changed to sometimes return None,
        // deal with that case in std::panicking::default_hook.
        Some(&self.location)
    }
}

/// A struct containing information about the location of a panic.
///
/// This structure is created by the [`location`] method of [`PanicInfo`].
///
/// [`location`]: ../../std/panic/struct.PanicInfo.html#method.location
/// [`PanicInfo`]: ../../std/panic/struct.PanicInfo.html
///
/// # Examples
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|panic_info| {
///     if let Some(location) = panic_info.location() {
///         println!("panic occurred in file '{}' at line {}", location.file(), location.line());
///     } else {
///         println!("panic occurred but can't get location information...");
///     }
/// }));
///
/// panic!("Normal panic");
/// ```
#[derive(Debug)]
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub struct Location<'a> {
    file: &'a str,
    line: u32,
    col: u32,
}

impl<'a> Location<'a> {
    #![unstable(feature = "panic_internals",
                reason = "internal details of the implementation of the `panic!` \
                          and related macros",
                issue = "0")]
    #[doc(hidden)]
    pub fn internal_constructor(file: &'a str, line: u32, col: u32) -> Self {
        Location { file, line, col }
    }

    /// Returns the name of the source file from which the panic originated.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}'", location.file());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn file(&self) -> &str {
        self.file
    }

    /// Returns the line number from which the panic originated.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred at line {}", location.line());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the column from which the panic originated.
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred at column {}", location.column());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[stable(feature = "panic_col", since = "1.25")]
    pub fn column(&self) -> u32 {
        self.col
    }
}
