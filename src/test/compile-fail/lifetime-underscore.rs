// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn _f<'_>() //~ ERROR invalid lifetime name `'_`
    -> &'_ u8 //~ ERROR invalid lifetime name `'_`
{
    panic!();
}

fn main() {
    '_: loop { //~ ERROR invalid label name `'_`
        break '_ //~ ERROR invalid label name `'_`
    }
}
