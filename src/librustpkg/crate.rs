// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::path::Path;
use core::vec;

/// A crate is a unit of Rust code to be compiled into a binary or library
pub struct Crate {
    file: Path,
    flags: ~[~str],
    cfgs: ~[~str]
}

impl Crate {

    pub fn new(p: &Path) -> Crate {
        Crate {
            file: copy *p,
            flags: ~[],
            cfgs: ~[]
        }
    }

    fn flag(&self, flag: ~str) -> Crate {
        Crate {
            flags: vec::append(copy self.flags, [flag]),
            .. copy *self
        }
    }

    fn flags(&self, flags: ~[~str]) -> Crate {
        Crate {
            flags: vec::append(copy self.flags, flags),
            .. copy *self
        }
    }

    fn cfg(&self, cfg: ~str) -> Crate {
        Crate {
            cfgs: vec::append(copy self.cfgs, [cfg]),
            .. copy *self
        }
    }

    fn cfgs(&self, cfgs: ~[~str]) -> Crate {
        Crate {
            cfgs: vec::append(copy self.cfgs, cfgs),
            .. copy *self
        }
    }
}
