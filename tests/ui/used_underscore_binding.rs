#![feature(plugin)]
#![plugin(clippy)]
#![deny(clippy)]

#![allow(blacklisted_name)]
#![deny(used_underscore_binding)]

macro_rules! test_macro {
    () => {{
        let _foo = 42;
        _foo + 1
    }}
}

/// Test that we lint if we use a binding with a single leading underscore
fn prefix_underscore(_foo: u32) -> u32 {
    _foo + 1
}

/// Test that we lint if we use a `_`-variable defined outside within a macro expansion
fn in_macro(_foo: u32) {
    println!("{}", _foo);
    assert_eq!(_foo, _foo);

    test_macro!() + 1;
}

// Struct for testing use of fields prefixed with an underscore
struct StructFieldTest {
    _underscore_field: u32,
}

/// Test that we lint the use of a struct field which is prefixed with an underscore
fn in_struct_field() {
    let mut s = StructFieldTest { _underscore_field: 0 };
    s._underscore_field += 1;
}

/// Test that we do not lint if the underscore is not a prefix
fn non_prefix_underscore(some_foo: u32) -> u32 {
    some_foo + 1
}

/// Test that we do not lint if we do not use the binding (simple case)
fn unused_underscore_simple(_foo: u32) -> u32 {
    1
}

/// Test that we do not lint if we do not use the binding (complex case). This checks for
/// compatibility with the built-in `unused_variables` lint.
fn unused_underscore_complex(mut _foo: u32) -> u32 {
    _foo += 1;
    _foo = 2;
    1
}

///Test that we do not lint for multiple underscores
fn multiple_underscores(__foo: u32) -> u32 {
    __foo + 1
}

// Non-variable bindings with preceding underscore
fn _fn_test() {}
struct _StructTest;
enum _EnumTest {
    _Empty,
    _Value(_StructTest)
}

/// Test that we do not lint for non-variable bindings
fn non_variables() {
    _fn_test();
    let _s = _StructTest;
    let _e = match _EnumTest::_Value(_StructTest) {
        _EnumTest::_Empty => 0,
        _EnumTest::_Value(_st) => 1,
    };
    let f = _fn_test;
    f();
}

fn main() {
    let foo = 0u32;
    // tests of unused_underscore lint
    let _ = prefix_underscore(foo);
    in_macro(foo);
    in_struct_field();
    // possible false positives
    let _ = non_prefix_underscore(foo);
    let _ = unused_underscore_simple(foo);
    let _ = unused_underscore_complex(foo);
    let _ = multiple_underscores(foo);
    non_variables();
}
