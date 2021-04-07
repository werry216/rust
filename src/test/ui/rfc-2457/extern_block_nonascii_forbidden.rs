#![feature(extern_types)]
#![feature(non_ascii_idents)]

extern "C" {
    type 一; //~ items in `extern` blocks cannot use non-ascii identifiers
    fn 二(); //~ items in `extern` blocks cannot use non-ascii identifiers
    static 三: usize; //~ items in `extern` blocks cannot use non-ascii identifiers
}

fn main() {}
