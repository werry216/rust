


#![warn(inline_fn_without_body)]
#![allow(inline_always)]
trait Foo {
    #[inline]
    fn default_inline();

    #[inline(always)]
    fn always_inline();

    #[inline]
    fn has_body() {
    }
}

fn main() {}
