// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




#![warn(clippy::module_inception)]

mod foo {
    mod bar {
        mod bar {
            mod foo {}
        }
        mod foo {}
    }
    mod foo {
        mod bar {}
    }
}

// No warning. See <https://github.com/rust-lang-nursery/rust-clippy/issues/1220>.
mod bar {
    #[allow(clippy::module_inception)]
    mod bar {
    }
}

fn main() {}
