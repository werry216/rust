//! Error types for conversion to integral types.

use crate::convert::Infallible;
use crate::fmt;

/// The error type returned when a checked integral type conversion fails.
#[stable(feature = "try_from", since = "1.34.0")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromIntError(pub(crate) ());

impl TryFromIntError {
    #[unstable(
        feature = "int_error_internals",
        reason = "available through Error trait and this method should \
                  not be exposed publicly",
        issue = "none"
    )]
    #[doc(hidden)]
    pub fn __description(&self) -> &str {
        "out of range integral type conversion attempted"
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl fmt::Display for TryFromIntError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.__description().fmt(fmt)
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl From<Infallible> for TryFromIntError {
    fn from(x: Infallible) -> TryFromIntError {
        match x {}
    }
}

#[unstable(feature = "never_type", issue = "35121")]
impl From<!> for TryFromIntError {
    fn from(never: !) -> TryFromIntError {
        // Match rather than coerce to make sure that code like
        // `From<Infallible> for TryFromIntError` above will keep working
        // when `Infallible` becomes an alias to `!`.
        match never {}
    }
}

/// An error which can be returned when parsing an integer.
///
/// This error is used as the error type for the `from_str_radix()` functions
/// on the primitive integer types, such as [`i8::from_str_radix`].
///
/// # Potential causes
///
/// Among other causes, `ParseIntError` can be thrown because of leading or trailing whitespace
/// in the string e.g., when it is obtained from the standard input.
/// Using the [`str.trim()`] method ensures that no whitespace remains before parsing.
///
/// [`str.trim()`]: ../../std/primitive.str.html#method.trim
/// [`i8::from_str_radix`]: ../../std/primitive.i8.html#method.from_str_radix
///
/// # Example
///
/// ```
/// if let Err(e) = i32::from_str_radix("a12", 10) {
///     println!("Failed conversion to i32: {}", e);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct ParseIntError {
    pub(super) kind: IntErrorKind,
}

/// Enum to store the various types of errors that can cause parsing an integer to fail.
///
/// # Example
///
/// ```
/// # fn main() {
/// if let Err(e) = i32::from_str_radix("a12", 10) {
///     println!("Failed conversion to i32: {:?}", e.kind());
/// }
/// # }
/// ```
#[stable(feature = "int_error_matching", since = "1.47.0")]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntErrorKind {
    /// Value being parsed is empty.
    ///
    /// Among other causes, this variant will be constructed when parsing an empty string.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    Empty,
    /// Contains an invalid digit.
    ///
    /// Among other causes, this variant will be constructed when parsing a string that
    /// contains a letter.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    InvalidDigit,
    /// Integer is too large to store in target integer type.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    PosOverflow,
    /// Integer is too small to store in target integer type.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    NegOverflow,
    /// Value was Zero
    ///
    /// This variant will be emitted when the parsing string has a value of zero, which
    /// would be illegal for non-zero types.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    Zero,
    /// The value contains nothing other than signs `+` or `-`.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    OnlySign,
}

impl ParseIntError {
    /// Outputs the detailed cause of parsing an integer failing.
    #[stable(feature = "int_error_matching", since = "1.47.0")]
    pub fn kind(&self) -> &IntErrorKind {
        &self.kind
    }
    #[unstable(
        feature = "int_error_internals",
        reason = "available through Error trait and this method should \
                  not be exposed publicly",
        issue = "none"
    )]
    #[doc(hidden)]
    pub fn __description(&self) -> &str {
        match self.kind {
            IntErrorKind::Empty => "cannot parse integer from empty string",
            IntErrorKind::InvalidDigit => "invalid digit found in string",
            IntErrorKind::PosOverflow => "number too large to fit in target type",
            IntErrorKind::NegOverflow => "number too small to fit in target type",
            IntErrorKind::Zero => "number would be zero for non-zero type",
            IntErrorKind::OnlySign => "only signs without digits found in string",
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for ParseIntError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.__description().fmt(f)
    }
}
