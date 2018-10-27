// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




#![warn(clippy::use_self)]
#![allow(dead_code)]
#![allow(clippy::should_implement_trait)]

fn main() {}

mod use_self {
    struct Foo {}

    impl Foo {
        fn new() -> Foo {
            Foo {}
        }
        fn test() -> Foo {
            Foo::new()
        }
    }

    impl Default for Foo {
        fn default() -> Foo {
            Foo::new()
        }
    }
}

mod better {
    struct Foo {}

    impl Foo {
        fn new() -> Self {
            Self {}
        }
        fn test() -> Self {
            Self::new()
        }
    }

    impl Default for Foo {
        fn default() -> Self {
            Self::new()
        }
    }
}

//todo the lint does not handle lifetimed struct
//the following module should trigger the lint on the third method only
mod lifetimes {
    struct Foo<'a>{foo_str: &'a str}

    impl<'a> Foo<'a> {
        // Cannot use `Self` as return type, because the function is actually `fn foo<'b>(s: &'b str) -> Foo<'b>`
        fn foo(s: &str) -> Foo {
            Foo { foo_str: s }
        }
        // cannot replace with `Self`, because that's `Foo<'a>`
        fn bar() -> Foo<'static> {
            Foo { foo_str: "foo"}
        }

        // `Self` is applicable here
        fn clone(&self) -> Foo<'a> {
            Foo {foo_str: self.foo_str}
        }
    }
}

#[allow(clippy::boxed_local)]
mod traits {

    use std::ops::Mul;

    trait SelfTrait {
        fn refs(p1: &Self) -> &Self;
        fn ref_refs<'a>(p1: &'a &'a Self) -> &'a &'a Self;
        fn mut_refs(p1: &mut Self) -> &mut Self;
        fn nested(p1: Box<Self>, p2: (&u8, &Self));
        fn vals(r: Self) -> Self;
    }

    #[derive(Default)]
    struct Bad;

    impl SelfTrait for Bad {
        fn refs(p1: &Bad) -> &Bad {
            p1
        }

        fn ref_refs<'a>(p1: &'a &'a Bad) -> &'a &'a Bad {
            p1
        }

        fn mut_refs(p1: &mut Bad) -> &mut Bad {
            p1
        }

        fn nested(_p1: Box<Bad>, _p2: (&u8, &Bad)) {
        }

        fn vals(_: Bad) -> Bad {
            Bad::default()
        }
    }

    impl Mul for Bad {
        type Output = Bad;

        fn mul(self, rhs: Bad) -> Bad {
            rhs
        }
    }

    #[derive(Default)]
    struct Good;

    impl SelfTrait for Good {
        fn refs(p1: &Self) -> &Self {
            p1
        }

        fn ref_refs<'a>(p1: &'a &'a Self) -> &'a &'a Self {
            p1
        }

        fn mut_refs(p1: &mut Self) -> &mut Self {
            p1
        }

        fn nested(_p1: Box<Self>, _p2: (&u8, &Self)) {
        }

        fn vals(_: Self) -> Self {
            Self::default()
        }
    }

    impl Mul for Good {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self {
            rhs
        }
    }

    trait NameTrait {
        fn refs(p1: &u8) -> &u8;
        fn ref_refs<'a>(p1: &'a &'a u8) -> &'a &'a u8;
        fn mut_refs(p1: &mut u8) -> &mut u8;
        fn nested(p1: Box<u8>, p2: (&u8, &u8));
        fn vals(p1: u8) -> u8;
    }

    // Using `Self` instead of the type name is OK
    impl NameTrait for u8 {
        fn refs(p1: &Self) -> &Self {
            p1
        }

        fn ref_refs<'a>(p1: &'a &'a Self) -> &'a &'a Self {
            p1
        }

        fn mut_refs(p1: &mut Self) -> &mut Self {
            p1
        }

        fn nested(_p1: Box<Self>, _p2: (&Self, &Self)) {
        }

        fn vals(_: Self) -> Self {
            Self::default()
        }
    }

    // Check that self arg isn't linted
    impl Clone for Good {
        fn clone(&self) -> Self {
            // Note: Not linted and it wouldn't be valid
            // because "can't use `Self` as a constructor`"
            Good
        }
    }
}

mod issue2894 {
    trait IntoBytes {
        fn into_bytes(&self) -> Vec<u8>;
    }

    // This should not be linted
    impl IntoBytes for u8 {
        fn into_bytes(&self) -> Vec<u8> {
            vec![*self]
        }
    }
}

mod existential {
    struct Foo;

    impl Foo {
        fn bad(foos: &[Self]) -> impl Iterator<Item=&Foo> {
            foos.iter()
        }

        fn good(foos: &[Self]) -> impl Iterator<Item=&Self> {
            foos.iter()
        }
    }
}
