#![crate_name = "foo"]

#![unstable(feature = "humans",
            reason = "who ever let humans program computers, we're apparently really bad at it",
            issue = "none")]

#![feature(foo, foo2)]
#![feature(staged_api)]

// @has 'foo/fn.foo.html' '//pre' 'pub fn foo() -> u32'
// @has - '//span[@class="since"]' '1.0.0 (const: unstable)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature="foo", issue = "none")]
pub const fn foo() -> u32 { 42 }

// @has 'foo/fn.foo_unsafe.html' '//pre' 'pub unsafe fn foo_unsafe() -> u32'
// @has - '//span[@class="since"]' '1.0.0 (const: unstable)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature="foo", issue = "none")]
pub const unsafe fn foo_unsafe() -> u32 { 42 }

// @has 'foo/fn.foo2.html' '//pre' 'pub const fn foo2() -> u32'
#[unstable(feature = "humans", issue = "none")]
pub const fn foo2() -> u32 { 42 }

// @has 'foo/fn.foo2_unsafe.html' '//pre' 'pub const unsafe fn foo2_unsafe() -> u32'
#[unstable(feature = "humans", issue = "none")]
pub const unsafe fn foo2_unsafe() -> u32 { 42 }

// @has 'foo/fn.bar2.html' '//pre' 'pub const fn bar2() -> u32'
// @has - //span '1.0.0 (const: 1.0.0)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "rust1", since = "1.0.0")]
pub const fn bar2() -> u32 { 42 }

// @has 'foo/fn.bar2_unsafe.html' '//pre' 'pub const unsafe fn bar2_unsafe() -> u32'
// @has - //span '1.0.0 (const: 1.0.0)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "rust1", since = "1.0.0")]
pub const unsafe fn bar2_unsafe() -> u32 { 42 }

// @has 'foo/fn.foo2_gated.html' '//pre' 'pub const fn foo2_gated() -> u32'
#[unstable(feature = "foo2", issue = "none")]
pub const fn foo2_gated() -> u32 { 42 }

// @has 'foo/fn.foo2_gated_unsafe.html' '//pre' 'pub const unsafe fn foo2_gated_unsafe() -> u32'
#[unstable(feature = "foo2", issue = "none")]
pub const unsafe fn foo2_gated_unsafe() -> u32 { 42 }

// @has 'foo/fn.bar2_gated.html' '//pre' 'pub const fn bar2_gated() -> u32'
// @has - '//span[@class="since"]' '1.0.0 (const: 1.0.0)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "rust1", since = "1.0.0")]
pub const fn bar2_gated() -> u32 { 42 }

// @has 'foo/fn.bar2_gated_unsafe.html' '//pre' 'pub const unsafe fn bar2_gated_unsafe() -> u32'
// @has - '//span[@class="since"]' '1.0.0 (const: 1.0.0)'
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "rust1", since = "1.0.0")]
pub const unsafe fn bar2_gated_unsafe() -> u32 { 42 }

// @has 'foo/fn.bar_not_gated.html' '//pre' 'pub const fn bar_not_gated() -> u32'
pub const fn bar_not_gated() -> u32 { 42 }

// @has 'foo/fn.bar_not_gated_unsafe.html' '//pre' 'pub const unsafe fn bar_not_gated_unsafe() -> u32'
pub const unsafe fn bar_not_gated_unsafe() -> u32 { 42 }

pub struct Foo;

impl Foo {
    // @has 'foo/struct.Foo.html' '//div[@id="method.gated"]/code' 'pub fn gated() -> u32'
    // @has - '//span[@class="since"]' '1.0.0 (const: unstable)'
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature="foo", issue = "none")]
    pub const fn gated() -> u32 { 42 }

    // @has 'foo/struct.Foo.html' '//div[@id="method.gated_unsafe"]/code' 'pub unsafe fn gated_unsafe() -> u32'
    // @has - '//span[@class="since"]' '1.0.0 (const: unstable)'
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature="foo", issue = "none")]
    pub const unsafe fn gated_unsafe() -> u32 { 42 }

    // @has 'foo/struct.Foo.html' '//div[@id="method.stable_impl"]/code' 'pub const fn stable_impl() -> u32'
    // @has - '//span[@class="since"]' '1.0.0 (const: 1.2.0)'
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "rust1", since = "1.2.0")]
    pub const fn stable_impl() -> u32 { 42 }

    // @has 'foo/struct.Foo.html' '//div[@id="method.stable_impl_unsafe"]/code' 'pub const unsafe fn stable_impl_unsafe() -> u32'
    // @has - '//span[@class="since"]' '1.0.0 (const: 1.2.0)'
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "rust1", since = "1.2.0")]
    pub const unsafe fn stable_impl_unsafe() -> u32 { 42 }
}
