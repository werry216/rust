// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.





#![warn(clippy::integer_arithmetic, clippy::float_arithmetic)]
#![allow(unused, clippy::shadow_reuse, clippy::shadow_unrelated, clippy::no_effect, clippy::unnecessary_operation)]
fn main() {
    let i = 1i32;
    1 + i;
    i * 2;
    1 %
    i / 2; // no error, this is part of the expression in the preceding line
    i - 2 + 2 - i;
    -i;

    i & 1; // no wrapping
    i | 1;
    i ^ 1;
    i >> 1;
    i << 1;

    let f = 1.0f32;

    f * 2.0;

    1.0 + f;
    f * 2.0;
    f / 2.0;
    f - 2.0 * 4.2;
    -f;

    // No errors for the following items because they are constant expressions
    enum Foo {
        Bar = -2,
    }
    struct Baz([i32; 1 + 1]);
    union Qux {
        field: [i32; 1 + 1],
    }
    type Alias = [i32; 1 + 1];

    const FOO: i32 = -2;
    static BAR: i32 = -2;

    let _: [i32; 1 + 1] = [0, 0];

    let _: [i32; 1 + 1] = {
        let a: [i32; 1 + 1] = [0, 0];
        a
    };

    trait Trait {
        const ASSOC: i32 = 1 + 1;
    }

    impl Trait for Foo {
        const ASSOC: i32 = {
            let _: [i32; 1 + 1];
            fn foo() {}
            1 + 1
        };
    }
}
