// check-pass
// run-rustfix

#![warn(ellipsis_inclusive_range_patterns)]

fn main() {
    let despondency = 2;
    match despondency {
        1...2 => {}
        //~^ WARN `...` range patterns are deprecated
        //~| WARN this was previously accepted by the compiler
        _ => {}
    }

    match &despondency {
        &1...2 => {}
        //~^ WARN `...` range patterns are deprecated
        //~| WARN this was previously accepted by the compiler
        _ => {}
    }
}
