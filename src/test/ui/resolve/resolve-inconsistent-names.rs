#![allow(non_camel_case_types)]

enum E { A, B, c }

mod m {
    const CONST1: usize = 10;
    const Const2: usize = 20;
}

fn main() {
    let y = 1;
    match y {
       a | b => {} //~  ERROR variable `a` is not bound in all patterns
                   //~| ERROR variable `b` is not bound in all patterns
    }

    let x = (E::A, E::B);
    match x {
        (A, B) | (ref B, c) | (c, A) => ()
        //~^ ERROR variable `A` is not bound in all patterns
        //~| ERROR variable `B` is not bound in all patterns
        //~| ERROR variable `B` is bound inconsistently
        //~| ERROR mismatched types
        //~| ERROR variable `c` is not bound in all patterns
        //~| HELP consider making the path in the pattern qualified: `?::A`
    }

    let z = (10, 20);
    match z {
        (CONST1, _) | (_, Const2) => ()
        //~^ ERROR variable `CONST1` is not bound in all patterns
        //~| HELP consider making the path in the pattern qualified: `?::CONST1`
        //~| ERROR variable `Const2` is not bound in all patterns
        //~| HELP consider making the path in the pattern qualified: `?::Const2`
    }
}
