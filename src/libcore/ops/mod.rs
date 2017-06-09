// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Overloadable operators.
//!
//! Implementing these traits allows you to overload certain operators.
//!
//! Some of these traits are imported by the prelude, so they are available in
//! every Rust program. Only operators backed by traits can be overloaded. For
//! example, the addition operator (`+`) can be overloaded through the [`Add`]
//! trait, but since the assignment operator (`=`) has no backing trait, there
//! is no way of overloading its semantics. Additionally, this module does not
//! provide any mechanism to create new operators. If traitless overloading or
//! custom operators are required, you should look toward macros or compiler
//! plugins to extend Rust's syntax.
//!
//! Note that the `&&` and `||` operators short-circuit, i.e. they only
//! evaluate their second operand if it contributes to the result. Since this
//! behavior is not enforceable by traits, `&&` and `||` are not supported as
//! overloadable operators.
//!
//! Many of the operators take their operands by value. In non-generic
//! contexts involving built-in types, this is usually not a problem.
//! However, using these operators in generic code, requires some
//! attention if values have to be reused as opposed to letting the operators
//! consume them. One option is to occasionally use [`clone`].
//! Another option is to rely on the types involved providing additional
//! operator implementations for references. For example, for a user-defined
//! type `T` which is supposed to support addition, it is probably a good
//! idea to have both `T` and `&T` implement the traits [`Add<T>`][`Add`] and
//! [`Add<&T>`][`Add`] so that generic code can be written without unnecessary
//! cloning.
//!
//! # Examples
//!
//! This example creates a `Point` struct that implements [`Add`] and [`Sub`],
//! and then demonstrates adding and subtracting two `Point`s.
//!
//! ```rust
//! use std::ops::{Add, Sub};
//!
//! #[derive(Debug)]
//! struct Point {
//!     x: i32,
//!     y: i32,
//! }
//!
//! impl Add for Point {
//!     type Output = Point;
//!
//!     fn add(self, other: Point) -> Point {
//!         Point {x: self.x + other.x, y: self.y + other.y}
//!     }
//! }
//!
//! impl Sub for Point {
//!     type Output = Point;
//!
//!     fn sub(self, other: Point) -> Point {
//!         Point {x: self.x - other.x, y: self.y - other.y}
//!     }
//! }
//! fn main() {
//!     println!("{:?}", Point {x: 1, y: 0} + Point {x: 2, y: 3});
//!     println!("{:?}", Point {x: 1, y: 0} - Point {x: 2, y: 3});
//! }
//! ```
//!
//! See the documentation for each trait for an example implementation.
//!
//! The [`Fn`], [`FnMut`], and [`FnOnce`] traits are implemented by types that can be
//! invoked like functions. Note that [`Fn`] takes `&self`, [`FnMut`] takes `&mut
//! self` and [`FnOnce`] takes `self`. These correspond to the three kinds of
//! methods that can be invoked on an instance: call-by-reference,
//! call-by-mutable-reference, and call-by-value. The most common use of these
//! traits is to act as bounds to higher-level functions that take functions or
//! closures as arguments.
//!
//! Taking a [`Fn`] as a parameter:
//!
//! ```rust
//! fn call_with_one<F>(func: F) -> usize
//!     where F: Fn(usize) -> usize
//! {
//!     func(1)
//! }
//!
//! let double = |x| x * 2;
//! assert_eq!(call_with_one(double), 2);
//! ```
//!
//! Taking a [`FnMut`] as a parameter:
//!
//! ```rust
//! fn do_twice<F>(mut func: F)
//!     where F: FnMut()
//! {
//!     func();
//!     func();
//! }
//!
//! let mut x: usize = 1;
//! {
//!     let add_two_to_x = || x += 2;
//!     do_twice(add_two_to_x);
//! }
//!
//! assert_eq!(x, 5);
//! ```
//!
//! Taking a [`FnOnce`] as a parameter:
//!
//! ```rust
//! fn consume_with_relish<F>(func: F)
//!     where F: FnOnce() -> String
//! {
//!     // `func` consumes its captured variables, so it cannot be run more
//!     // than once
//!     println!("Consumed: {}", func());
//!
//!     println!("Delicious!");
//!
//!     // Attempting to invoke `func()` again will throw a `use of moved
//!     // value` error for `func`
//! }
//!
//! let x = String::from("x");
//! let consume_and_return_x = move || x;
//! consume_with_relish(consume_and_return_x);
//!
//! // `consume_and_return_x` can no longer be invoked at this point
//! ```
//!
//! [`Fn`]: trait.Fn.html
//! [`FnMut`]: trait.FnMut.html
//! [`FnOnce`]: trait.FnOnce.html
//! [`Add`]: trait.Add.html
//! [`Sub`]: trait.Sub.html
//! [`clone`]: ../clone/trait.Clone.html#tymethod.clone

#![stable(feature = "rust1", since = "1.0.0")]

mod arith;
mod bit;
mod function;
mod range;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::arith::{Add, Sub, Mul, Div, Rem, Neg};

#[stable(feature = "op_assign_traits", since = "1.8.0")]
pub use self::arith::{AddAssign, SubAssign, MulAssign, DivAssign, RemAssign};

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::bit::{Not, BitAnd, BitOr, BitXor, Shl, Shr};

#[stable(feature = "op_assign_traits", since = "1.8.0")]
pub use self::bit::{BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign};

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::function::{Fn, FnMut, FnOnce};

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::range::{Range, RangeFrom, RangeFull, RangeTo};

#[unstable(feature = "inclusive_range", reason = "recently added, follows RFC", issue = "28237")]
pub use self::range::{RangeInclusive, RangeToInclusive};

use marker::Unsize;

/// The `Drop` trait is used to run some code when a value goes out of scope.
/// This is sometimes called a 'destructor'.
///
/// When a value goes out of scope, if it implements this trait, it will have
/// its `drop` method called. Then any fields the value contains will also
/// be dropped recursively.
///
/// Because of the recursive dropping, you do not need to implement this trait
/// unless your type needs its own destructor logic.
///
/// # Examples
///
/// A trivial implementation of `Drop`. The `drop` method is called when `_x`
/// goes out of scope, and therefore `main` prints `Dropping!`.
///
/// ```
/// struct HasDrop;
///
/// impl Drop for HasDrop {
///     fn drop(&mut self) {
///         println!("Dropping!");
///     }
/// }
///
/// fn main() {
///     let _x = HasDrop;
/// }
/// ```
///
/// Showing the recursive nature of `Drop`. When `outer` goes out of scope, the
/// `drop` method will be called first for `Outer`, then for `Inner`. Therefore
/// `main` prints `Dropping Outer!` and then `Dropping Inner!`.
///
/// ```
/// struct Inner;
/// struct Outer(Inner);
///
/// impl Drop for Inner {
///     fn drop(&mut self) {
///         println!("Dropping Inner!");
///     }
/// }
///
/// impl Drop for Outer {
///     fn drop(&mut self) {
///         println!("Dropping Outer!");
///     }
/// }
///
/// fn main() {
///     let _x = Outer(Inner);
/// }
/// ```
///
/// Because variables are dropped in the reverse order they are declared,
/// `main` will print `Declared second!` and then `Declared first!`.
///
/// ```
/// struct PrintOnDrop(&'static str);
///
/// fn main() {
///     let _first = PrintOnDrop("Declared first!");
///     let _second = PrintOnDrop("Declared second!");
/// }
/// ```
#[lang = "drop"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Drop {
    /// A method called when the value goes out of scope.
    ///
    /// When this method has been called, `self` has not yet been deallocated.
    /// If it were, `self` would be a dangling reference.
    ///
    /// After this function is over, the memory of `self` will be deallocated.
    ///
    /// This function cannot be called explicitly. This is compiler error
    /// [E0040]. However, the [`std::mem::drop`] function in the prelude can be
    /// used to call the argument's `Drop` implementation.
    ///
    /// [E0040]: ../../error-index.html#E0040
    /// [`std::mem::drop`]: ../../std/mem/fn.drop.html
    ///
    /// # Panics
    ///
    /// Given that a `panic!` will call `drop()` as it unwinds, any `panic!` in
    /// a `drop()` implementation will likely abort.
    #[stable(feature = "rust1", since = "1.0.0")]
    fn drop(&mut self);
}

/// The `Index` trait is used to specify the functionality of indexing operations
/// like `container[index]` when used in an immutable context.
///
/// `container[index]` is actually syntactic sugar for `*container.index(index)`,
/// but only when used as an immutable value. If a mutable value is requested,
/// [`IndexMut`] is used instead. This allows nice things such as
/// `let value = v[index]` if `value` implements [`Copy`].
///
/// [`IndexMut`]: ../../std/ops/trait.IndexMut.html
/// [`Copy`]: ../../std/marker/trait.Copy.html
///
/// # Examples
///
/// The following example implements `Index` on a read-only `NucleotideCount`
/// container, enabling individual counts to be retrieved with index syntax.
///
/// ```
/// use std::ops::Index;
///
/// enum Nucleotide {
///     A,
///     C,
///     G,
///     T,
/// }
///
/// struct NucleotideCount {
///     a: usize,
///     c: usize,
///     g: usize,
///     t: usize,
/// }
///
/// impl Index<Nucleotide> for NucleotideCount {
///     type Output = usize;
///
///     fn index(&self, nucleotide: Nucleotide) -> &usize {
///         match nucleotide {
///             Nucleotide::A => &self.a,
///             Nucleotide::C => &self.c,
///             Nucleotide::G => &self.g,
///             Nucleotide::T => &self.t,
///         }
///     }
/// }
///
/// let nucleotide_count = NucleotideCount {a: 14, c: 9, g: 10, t: 12};
/// assert_eq!(nucleotide_count[Nucleotide::A], 14);
/// assert_eq!(nucleotide_count[Nucleotide::C], 9);
/// assert_eq!(nucleotide_count[Nucleotide::G], 10);
/// assert_eq!(nucleotide_count[Nucleotide::T], 12);
/// ```
#[lang = "index"]
#[rustc_on_unimplemented = "the type `{Self}` cannot be indexed by `{Idx}`"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Index<Idx: ?Sized> {
    /// The returned type after indexing
    #[stable(feature = "rust1", since = "1.0.0")]
    type Output: ?Sized;

    /// The method for the indexing (`container[index]`) operation
    #[stable(feature = "rust1", since = "1.0.0")]
    fn index(&self, index: Idx) -> &Self::Output;
}

/// The `IndexMut` trait is used to specify the functionality of indexing
/// operations like `container[index]` when used in a mutable context.
///
/// `container[index]` is actually syntactic sugar for
/// `*container.index_mut(index)`, but only when used as a mutable value. If
/// an immutable value is requested, the [`Index`] trait is used instead. This
/// allows nice things such as `v[index] = value` if `value` implements [`Copy`].
///
/// [`Index`]: ../../std/ops/trait.Index.html
/// [`Copy`]: ../../std/marker/trait.Copy.html
///
/// # Examples
///
/// A very simple implementation of a `Balance` struct that has two sides, where
/// each can be indexed mutably and immutably.
///
/// ```
/// use std::ops::{Index,IndexMut};
///
/// #[derive(Debug)]
/// enum Side {
///     Left,
///     Right,
/// }
///
/// #[derive(Debug, PartialEq)]
/// enum Weight {
///     Kilogram(f32),
///     Pound(f32),
/// }
///
/// struct Balance {
///     pub left: Weight,
///     pub right:Weight,
/// }
///
/// impl Index<Side> for Balance {
///     type Output = Weight;
///
///     fn index<'a>(&'a self, index: Side) -> &'a Weight {
///         println!("Accessing {:?}-side of balance immutably", index);
///         match index {
///             Side::Left => &self.left,
///             Side::Right => &self.right,
///         }
///     }
/// }
///
/// impl IndexMut<Side> for Balance {
///     fn index_mut<'a>(&'a mut self, index: Side) -> &'a mut Weight {
///         println!("Accessing {:?}-side of balance mutably", index);
///         match index {
///             Side::Left => &mut self.left,
///             Side::Right => &mut self.right,
///         }
///     }
/// }
///
/// fn main() {
///     let mut balance = Balance {
///         right: Weight::Kilogram(2.5),
///         left: Weight::Pound(1.5),
///     };
///
///     // In this case balance[Side::Right] is sugar for
///     // *balance.index(Side::Right), since we are only reading
///     // balance[Side::Right], not writing it.
///     assert_eq!(balance[Side::Right],Weight::Kilogram(2.5));
///
///     // However in this case balance[Side::Left] is sugar for
///     // *balance.index_mut(Side::Left), since we are writing
///     // balance[Side::Left].
///     balance[Side::Left] = Weight::Kilogram(3.0);
/// }
/// ```
#[lang = "index_mut"]
#[rustc_on_unimplemented = "the type `{Self}` cannot be mutably indexed by `{Idx}`"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait IndexMut<Idx: ?Sized>: Index<Idx> {
    /// The method for the mutable indexing (`container[index]`) operation
    #[stable(feature = "rust1", since = "1.0.0")]
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output;
}

/// The `Deref` trait is used to specify the functionality of dereferencing
/// operations, like `*v`.
///
/// `Deref` also enables ['`Deref` coercions'][coercions].
///
/// [coercions]: ../../book/deref-coercions.html
///
/// # Examples
///
/// A struct with a single field which is accessible via dereferencing the
/// struct.
///
/// ```
/// use std::ops::Deref;
///
/// struct DerefExample<T> {
///     value: T
/// }
///
/// impl<T> Deref for DerefExample<T> {
///     type Target = T;
///
///     fn deref(&self) -> &T {
///         &self.value
///     }
/// }
///
/// fn main() {
///     let x = DerefExample { value: 'a' };
///     assert_eq!('a', *x);
/// }
/// ```
#[lang = "deref"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Deref {
    /// The resulting type after dereferencing
    #[stable(feature = "rust1", since = "1.0.0")]
    type Target: ?Sized;

    /// The method called to dereference a value
    #[stable(feature = "rust1", since = "1.0.0")]
    fn deref(&self) -> &Self::Target;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T: ?Sized> Deref for &'a T {
    type Target = T;

    fn deref(&self) -> &T { *self }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T: ?Sized> Deref for &'a mut T {
    type Target = T;

    fn deref(&self) -> &T { *self }
}

/// The `DerefMut` trait is used to specify the functionality of dereferencing
/// mutably like `*v = 1;`
///
/// `DerefMut` also enables ['`Deref` coercions'][coercions].
///
/// [coercions]: ../../book/deref-coercions.html
///
/// # Examples
///
/// A struct with a single field which is modifiable via dereferencing the
/// struct.
///
/// ```
/// use std::ops::{Deref, DerefMut};
///
/// struct DerefMutExample<T> {
///     value: T
/// }
///
/// impl<T> Deref for DerefMutExample<T> {
///     type Target = T;
///
///     fn deref(&self) -> &T {
///         &self.value
///     }
/// }
///
/// impl<T> DerefMut for DerefMutExample<T> {
///     fn deref_mut(&mut self) -> &mut T {
///         &mut self.value
///     }
/// }
///
/// fn main() {
///     let mut x = DerefMutExample { value: 'a' };
///     *x = 'b';
///     assert_eq!('b', *x);
/// }
/// ```
#[lang = "deref_mut"]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait DerefMut: Deref {
    /// The method called to mutably dereference a value
    #[stable(feature = "rust1", since = "1.0.0")]
    fn deref_mut(&mut self) -> &mut Self::Target;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T: ?Sized> DerefMut for &'a mut T {
    fn deref_mut(&mut self) -> &mut T { *self }
}

/// Trait that indicates that this is a pointer or a wrapper for one,
/// where unsizing can be performed on the pointee.
///
/// See the [DST coercion RfC][dst-coerce] and [the nomicon entry on coercion][nomicon-coerce]
/// for more details.
///
/// For builtin pointer types, pointers to `T` will coerce to pointers to `U` if `T: Unsize<U>`
/// by converting from a thin pointer to a fat pointer.
///
/// For custom types, the coercion here works by coercing `Foo<T>` to `Foo<U>`
/// provided an impl of `CoerceUnsized<Foo<U>> for Foo<T>` exists.
/// Such an impl can only be written if `Foo<T>` has only a single non-phantomdata
/// field involving `T`. If the type of that field is `Bar<T>`, an implementation
/// of `CoerceUnsized<Bar<U>> for Bar<T>` must exist. The coercion will work by
/// by coercing the `Bar<T>` field into `Bar<U>` and filling in the rest of the fields
/// from `Foo<T>` to create a `Foo<U>`. This will effectively drill down to a pointer
/// field and coerce that.
///
/// Generally, for smart pointers you will implement
/// `CoerceUnsized<Ptr<U>> for Ptr<T> where T: Unsize<U>, U: ?Sized`, with an
/// optional `?Sized` bound on `T` itself. For wrapper types that directly embed `T`
/// like `Cell<T>` and `RefCell<T>`, you
/// can directly implement `CoerceUnsized<Wrap<U>> for Wrap<T> where T: CoerceUnsized<U>`.
/// This will let coercions of types like `Cell<Box<T>>` work.
///
/// [`Unsize`][unsize] is used to mark types which can be coerced to DSTs if behind
/// pointers. It is implemented automatically by the compiler.
///
/// [dst-coerce]: https://github.com/rust-lang/rfcs/blob/master/text/0982-dst-coercion.md
/// [unsize]: ../marker/trait.Unsize.html
/// [nomicon-coerce]: ../../nomicon/coercions.html
#[unstable(feature = "coerce_unsized", issue = "27732")]
#[lang="coerce_unsized"]
pub trait CoerceUnsized<T> {
    // Empty.
}

// &mut T -> &mut U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<&'a mut U> for &'a mut T {}
// &mut T -> &U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, 'b: 'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<&'a U> for &'b mut T {}
// &mut T -> *mut U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*mut U> for &'a mut T {}
// &mut T -> *const U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*const U> for &'a mut T {}

// &T -> &U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, 'b: 'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<&'a U> for &'b T {}
// &T -> *const U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<'a, T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*const U> for &'a T {}

// *mut T -> *mut U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*mut U> for *mut T {}
// *mut T -> *const U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*const U> for *mut T {}

// *const T -> *const U
#[unstable(feature = "coerce_unsized", issue = "27732")]
impl<T: ?Sized+Unsize<U>, U: ?Sized> CoerceUnsized<*const U> for *const T {}

/// Both `PLACE <- EXPR` and `box EXPR` desugar into expressions
/// that allocate an intermediate "place" that holds uninitialized
/// state.  The desugaring evaluates EXPR, and writes the result at
/// the address returned by the `pointer` method of this trait.
///
/// A `Place` can be thought of as a special representation for a
/// hypothetical `&uninit` reference (which Rust cannot currently
/// express directly). That is, it represents a pointer to
/// uninitialized storage.
///
/// The client is responsible for two steps: First, initializing the
/// payload (it can access its address via `pointer`). Second,
/// converting the agent to an instance of the owning pointer, via the
/// appropriate `finalize` method (see the `InPlace`.
///
/// If evaluating EXPR fails, then it is up to the destructor for the
/// implementation of Place to clean up any intermediate state
/// (e.g. deallocate box storage, pop a stack, etc).
#[unstable(feature = "placement_new_protocol", issue = "27779")]
pub trait Place<Data: ?Sized> {
    /// Returns the address where the input value will be written.
    /// Note that the data at this address is generally uninitialized,
    /// and thus one should use `ptr::write` for initializing it.
    fn pointer(&mut self) -> *mut Data;
}

/// Interface to implementations of  `PLACE <- EXPR`.
///
/// `PLACE <- EXPR` effectively desugars into:
///
/// ```rust,ignore
/// let p = PLACE;
/// let mut place = Placer::make_place(p);
/// let raw_place = Place::pointer(&mut place);
/// let value = EXPR;
/// unsafe {
///     std::ptr::write(raw_place, value);
///     InPlace::finalize(place)
/// }
/// ```
///
/// The type of `PLACE <- EXPR` is derived from the type of `PLACE`;
/// if the type of `PLACE` is `P`, then the final type of the whole
/// expression is `P::Place::Owner` (see the `InPlace` and `Boxed`
/// traits).
///
/// Values for types implementing this trait usually are transient
/// intermediate values (e.g. the return value of `Vec::emplace_back`)
/// or `Copy`, since the `make_place` method takes `self` by value.
#[unstable(feature = "placement_new_protocol", issue = "27779")]
pub trait Placer<Data: ?Sized> {
    /// `Place` is the intermedate agent guarding the
    /// uninitialized state for `Data`.
    type Place: InPlace<Data>;

    /// Creates a fresh place from `self`.
    fn make_place(self) -> Self::Place;
}

/// Specialization of `Place` trait supporting `PLACE <- EXPR`.
#[unstable(feature = "placement_new_protocol", issue = "27779")]
pub trait InPlace<Data: ?Sized>: Place<Data> {
    /// `Owner` is the type of the end value of `PLACE <- EXPR`
    ///
    /// Note that when `PLACE <- EXPR` is solely used for
    /// side-effecting an existing data-structure,
    /// e.g. `Vec::emplace_back`, then `Owner` need not carry any
    /// information at all (e.g. it can be the unit type `()` in that
    /// case).
    type Owner;

    /// Converts self into the final value, shifting
    /// deallocation/cleanup responsibilities (if any remain), over to
    /// the returned instance of `Owner` and forgetting self.
    unsafe fn finalize(self) -> Self::Owner;
}

/// Core trait for the `box EXPR` form.
///
/// `box EXPR` effectively desugars into:
///
/// ```rust,ignore
/// let mut place = BoxPlace::make_place();
/// let raw_place = Place::pointer(&mut place);
/// let value = EXPR;
/// unsafe {
///     ::std::ptr::write(raw_place, value);
///     Boxed::finalize(place)
/// }
/// ```
///
/// The type of `box EXPR` is supplied from its surrounding
/// context; in the above expansion, the result type `T` is used
/// to determine which implementation of `Boxed` to use, and that
/// `<T as Boxed>` in turn dictates determines which
/// implementation of `BoxPlace` to use, namely:
/// `<<T as Boxed>::Place as BoxPlace>`.
#[unstable(feature = "placement_new_protocol", issue = "27779")]
pub trait Boxed {
    /// The kind of data that is stored in this kind of box.
    type Data;  /* (`Data` unused b/c cannot yet express below bound.) */
    /// The place that will negotiate the storage of the data.
    type Place: BoxPlace<Self::Data>;

    /// Converts filled place into final owning value, shifting
    /// deallocation/cleanup responsibilities (if any remain), over to
    /// returned instance of `Self` and forgetting `filled`.
    unsafe fn finalize(filled: Self::Place) -> Self;
}

/// Specialization of `Place` trait supporting `box EXPR`.
#[unstable(feature = "placement_new_protocol", issue = "27779")]
pub trait BoxPlace<Data: ?Sized> : Place<Data> {
    /// Creates a globally fresh place.
    fn make_place() -> Self;
}

/// This trait has been superseded by the `Try` trait, but must remain
/// here as `?` is still lowered to it in stage0 .
#[cfg(stage0)]
#[unstable(feature = "question_mark_carrier", issue = "31436")]
pub trait Carrier {
    /// The type of the value when computation succeeds.
    type Success;
    /// The type of the value when computation errors out.
    type Error;

    /// Create a `Carrier` from a success value.
    fn from_success(_: Self::Success) -> Self;

    /// Create a `Carrier` from an error value.
    fn from_error(_: Self::Error) -> Self;

    /// Translate this `Carrier` to another implementation of `Carrier` with the
    /// same associated types.
    fn translate<T>(self) -> T where T: Carrier<Success=Self::Success, Error=Self::Error>;
}

#[cfg(stage0)]
#[unstable(feature = "question_mark_carrier", issue = "31436")]
impl<U, V> Carrier for Result<U, V> {
    type Success = U;
    type Error = V;

    fn from_success(u: U) -> Result<U, V> {
        Ok(u)
    }

    fn from_error(e: V) -> Result<U, V> {
        Err(e)
    }

    fn translate<T>(self) -> T
        where T: Carrier<Success=U, Error=V>
    {
        match self {
            Ok(u) => T::from_success(u),
            Err(e) => T::from_error(e),
        }
    }
}

struct _DummyErrorType;

impl Try for _DummyErrorType {
    type Ok = ();
    type Error = ();

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn from_ok(_: ()) -> _DummyErrorType {
        _DummyErrorType
    }

    fn from_error(_: ()) -> _DummyErrorType {
        _DummyErrorType
    }
}

/// A trait for customizing the behaviour of the `?` operator.
///
/// A type implementing `Try` is one that has a canonical way to view it
/// in terms of a success/failure dichotomy.  This trait allows both
/// extracting those success or failure values from an existing instance and
/// creating a new instance from a success or failure value.
#[unstable(feature = "try_trait", issue = "42327")]
pub trait Try {
    /// The type of this value when viewed as successful.
    #[unstable(feature = "try_trait", issue = "42327")]
    type Ok;
    /// The type of this value when viewed as failed.
    #[unstable(feature = "try_trait", issue = "42327")]
    type Error;

    /// Applies the "?" operator. A return of `Ok(t)` means that the
    /// execution should continue normally, and the result of `?` is the
    /// value `t`. A return of `Err(e)` means that execution should branch
    /// to the innermost enclosing `catch`, or return from the function.
    ///
    /// If an `Err(e)` result is returned, the value `e` will be "wrapped"
    /// in the return type of the enclosing scope (which must itself implement
    /// `Try`). Specifically, the value `X::from_error(From::from(e))`
    /// is returned, where `X` is the return type of the enclosing function.
    #[unstable(feature = "try_trait", issue = "42327")]
    fn into_result(self) -> Result<Self::Ok, Self::Error>;

    /// Wrap an error value to construct the composite result. For example,
    /// `Result::Err(x)` and `Result::from_error(x)` are equivalent.
    #[unstable(feature = "try_trait", issue = "42327")]
    fn from_error(v: Self::Error) -> Self;

    /// Wrap an OK value to construct the composite result. For example,
    /// `Result::Ok(x)` and `Result::from_ok(x)` are equivalent.
    #[unstable(feature = "try_trait", issue = "42327")]
    fn from_ok(v: Self::Ok) -> Self;
}
