// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(associated_consts)]
#![feature(associated_type_defaults)]

struct S;

mod method {
    trait A {
        fn a(&self) { }
    }

    pub trait B {
        fn b(&self) { }
    }

    pub trait C: A + B {
        fn c(&self) { }
    }

    impl A for ::S {}
    impl B for ::S {}
    impl C for ::S {}
}

mod assoc_const {
    trait A {
        const A: u8 = 0;
    }

    pub trait B {
        const B: u8 = 0;
    }

    pub trait C: A + B {
        const C: u8 = 0;
    }

    impl A for ::S {}
    impl B for ::S {}
    impl C for ::S {}
}

mod assoc_ty {
    trait A {
        type A = u8;
    }

    pub trait B {
        type B = u8;
    }

    pub trait C: A + B {
        type C = u8;
    }

    impl A for ::S {}
    impl B for ::S {}
    impl C for ::S {}
}

fn check_method() {
    // A is private
    // B is pub, not in scope
    // C : A + B is pub, in scope
    use method::C;

    // Methods, method call
    // a, b, c are resolved as trait items, their traits need to be in scope
    S.a(); //~ ERROR no method named `a` found for type `S` in the current scope
    S.b(); //~ ERROR no method named `b` found for type `S` in the current scope
    S.c(); // OK
    // a, b, c are resolved as inherent items, their traits don't need to be in scope
    let c = &S as &C;
    c.a(); //~ ERROR method `a` is private
    c.b(); // OK
    c.c(); // OK

    // Methods, UFCS
    // a, b, c are resolved as trait items, their traits need to be in scope
    S::a(&S);
    //~^ ERROR no function or associated item named `a` found for type `S`
    S::b(&S);
    //~^ ERROR no function or associated item named `b` found for type `S`
    S::c(&S); // OK
    // a, b, c are resolved as inherent items, their traits don't need to be in scope
    C::a(&S); //~ ERROR method `a` is private
    C::b(&S); // OK
    C::c(&S); // OK
}

fn check_assoc_const() {
    // A is private
    // B is pub, not in scope
    // C : A + B is pub, in scope
    use assoc_const::C;

    // Associated constants
    // A, B, C are resolved as trait items, their traits need to be in scope
    S::A; //~ ERROR no associated item named `A` found for type `S` in the current scope
    S::B; //~ ERROR no associated item named `B` found for type `S` in the current scope
    S::C; // OK
    // A, B, C are resolved as inherent items, their traits don't need to be in scope
    C::A; //~ ERROR associated constant `A` is private
          //~^ ERROR the trait `assoc_const::C` cannot be made into an object
          //~| ERROR the trait bound `assoc_const::C: assoc_const::A` is not satisfied
    C::B; // ERROR the trait `assoc_const::C` cannot be made into an object
          //~^ ERROR the trait bound `assoc_const::C: assoc_const::B` is not satisfied
    C::C; // OK
}

fn check_assoc_ty<T: assoc_ty::C>() {
    // A is private
    // B is pub, not in scope
    // C : A + B is pub, in scope
    use assoc_ty::C;

    // Associated types
    // A, B, C are resolved as trait items, their traits need to be in scope, not implemented yet
    let _: S::A; //~ ERROR ambiguous associated type
    let _: S::B; //~ ERROR ambiguous associated type
    let _: S::C; //~ ERROR ambiguous associated type
    // A, B, C are resolved as inherent items, their traits don't need to be in scope
    let _: T::A; //~ ERROR associated type `A` is private
    let _: T::B; // OK
    let _: T::C; // OK
}

fn main() {}
