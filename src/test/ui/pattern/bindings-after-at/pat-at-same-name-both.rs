// Test that `binding @ subpat` acts as a product context with respect to duplicate binding names.
// The code that is tested here lives in resolve (see `resolve_pattern_inner`).

#![feature(bindings_after_at)]
//~^ WARN the feature `bindings_after_at` is incomplete and may cause the compiler to crash
#![feature(or_patterns)]
//~^ WARN the feature `or_patterns` is incomplete and may cause the compiler to crash

fn main() {
    let a @ a @ a = ();
    //~^ ERROR identifier `a` is bound more than once in the same pattern
    //~| ERROR identifier `a` is bound more than once in the same pattern
    let ref a @ ref a = ();
    //~^ ERROR identifier `a` is bound more than once in the same pattern
    let ref mut a @ ref mut a = ();
    //~^ ERROR identifier `a` is bound more than once in the same pattern

    let a @ (Ok(a) | Err(a)) = Ok(());
    //~^ ERROR identifier `a` is bound more than once in the same pattern
    //~| ERROR identifier `a` is bound more than once in the same pattern
}
