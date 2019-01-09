#![allow(dead_code)]

// Test that even when `T` is only used in covariant position, it
// is treated as invariant.

trait Get<T> : 'static {
    fn get(&self) -> T;
}

fn get_min_from_max<'min, 'max>(v: Box<Get<&'max i32>>)
                                -> Box<Get<&'min i32>>
    where 'max : 'min
{
    // Previously OK, now an error as traits are invariant.
    v //~ ERROR mismatched types
}

fn get_max_from_min<'min, 'max, G>(v: Box<Get<&'min i32>>)
                                   -> Box<Get<&'max i32>>
    where 'max : 'min
{
    v //~ ERROR mismatched types
}

fn main() { }
