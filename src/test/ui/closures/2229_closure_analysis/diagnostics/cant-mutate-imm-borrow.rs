// Test that if we deref an immutable borrow to access a Place,
// then we can't mutate the final place.

#![feature(capture_disjoint_fields)]
//~^ WARNING: the feature `capture_disjoint_fields` is incomplete

fn main() {
    let mut x = (format!(""), format!("X2"));
    let mut y = (&x, "Y");
    let z = (&mut y, "Z");

    // `x.0` is mutable but we access `x` via `z.0.0`, which is an immutable reference and
    // therefore can't be mutated.
    let mut c = || {
    //~^ ERROR: cannot borrow `z.0.0.0` as mutable, as it is behind a `&` reference
        z.0.0.0 = format!("X1");
        //~^ ERROR: cannot assign to `z`, as it is not declared as mutable
    };

    c();
}
