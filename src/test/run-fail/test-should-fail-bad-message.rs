// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// check-stdout
// error-pattern:task 'test_foo' panicked at
// compile-flags: --test
// ignore-pretty: does not work well with `--test`

#[test]
#[should_fail(expected = "foobar")]
fn test_foo() {
    panic!("blah")
}


