// Test that the types of distinct fn items are not compatible by
// default. See also `run-pass/fn-item-type-*.rs`.

fn foo<T>(x: isize) -> isize { x * 2 }
fn bar<T>(x: isize) -> isize { x * 4 }

fn eq<T>(x: T, y: T) { }

trait Foo { fn foo() { /* this is a default fn */ } }
impl<T> Foo for T { /* `foo` is still default here */ }

fn main() {
    eq(foo::<u8>, bar::<u8>);
    //~^ ERROR mismatched types
    //~| expected fn item `fn(_) -> _ {foo::<u8>}`
    //~| found fn item `fn(_) -> _ {bar::<u8>}`
    //~| expected fn item, found a different fn item
    //~| different `fn` items always have unique types, even if their signatures are the same
    //~| change the expected type to be function pointer
    //~| if the expected type is due to type inference, cast the expected `fn` to a function pointer

    eq(foo::<u8>, foo::<i8>);
    //~^ ERROR mismatched types
    //~| expected `u8`, found `i8`
    //~| different `fn` items always have unique types, even if their signatures are the same
    //~| change the expected type to be function pointer
    //~| if the expected type is due to type inference, cast the expected `fn` to a function pointer

    eq(bar::<String>, bar::<Vec<u8>>);
    //~^ ERROR mismatched types
    //~| expected fn item `fn(_) -> _ {bar::<std::string::String>}`
    //~| found fn item `fn(_) -> _ {bar::<std::vec::Vec<u8>>}`
    //~| expected struct `std::string::String`, found struct `std::vec::Vec`
    //~| different `fn` items always have unique types, even if their signatures are the same
    //~| change the expected type to be function pointer
    //~| if the expected type is due to type inference, cast the expected `fn` to a function pointer

    // Make sure we distinguish between trait methods correctly.
    eq(<u8 as Foo>::foo, <u16 as Foo>::foo);
    //~^ ERROR mismatched types
    //~| expected `u8`, found `u16`
    //~| different `fn` items always have unique types, even if their signatures are the same
    //~| change the expected type to be function pointer
    //~| if the expected type is due to type inference, cast the expected `fn` to a function pointer

    eq(foo::<u8>, bar::<u8> as fn(isize) -> isize);
    //~^ ERROR mismatched types
    //~| expected fn item `fn(_) -> _ {foo::<u8>}`
    //~| found fn pointer `fn(_) -> _`
    //~| expected fn item, found fn pointer
    //~| change the expected type to be function pointer
    //~| if the expected type is due to type inference, cast the expected `fn` to a function pointer

    eq(foo::<u8> as fn(isize) -> isize, bar::<u8>); // ok!
}
