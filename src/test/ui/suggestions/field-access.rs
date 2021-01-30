// run-rustfix
#![allow(dead_code)]

struct A {
    b: B,
}

enum B {
    Fst,
    Snd,
}

fn main() {
    let a = A { b: B::Fst };
    if let B::Fst = a {}; //~ ERROR mismatched types [E0308]
    //~^ HELP you might have meant to use field `b` of type `B`
    match a {
        //~^ HELP you might have meant to use field `b` of type `B`
        //~| HELP you might have meant to use field `b` of type `B`
        B::Fst => (), //~ ERROR mismatched types [E0308]
        B::Snd => (), //~ ERROR mismatched types [E0308]
    }
}
