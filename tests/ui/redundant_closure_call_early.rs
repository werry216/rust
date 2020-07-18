// non rustfixable, see redundant_closure_call_fixable.rs

#![warn(clippy::redundant_closure_call)]

fn main() {
    let mut i = 1;

    // lint here
    let mut k = (|m| m + 1)(i);

    // lint here
    k = (|a, b| a * b)(1, 5);

    // don't lint these
    #[allow(clippy::needless_return)]
    (|| return 2)();
    (|| -> Option<i32> { None? })();
    (|| -> Result<i32, i32> { Err(2)? })();
}
