#![feature(box_patterns)]

#![feature(or_patterns)]
//~^ WARN the feature `or_patterns` is incomplete

#![allow(ellipsis_inclusive_range_patterns)]
#![allow(unreachable_patterns)]
#![allow(unused_variables)]
#![deny(unused_parens)]

fn lint_on_top_level() {
    let (a) = 0; //~ ERROR unnecessary parentheses around pattern
    for (a) in 0..1 {} //~ ERROR unnecessary parentheses around pattern
    if let (a) = 0 {} //~ ERROR unnecessary parentheses around pattern
    while let (a) = 0 {} //~ ERROR unnecessary parentheses around pattern
    fn foo((a): u8) {} //~ ERROR unnecessary parentheses around pattern
    let _ = |(a): u8| 0; //~ ERROR unnecessary parentheses around pattern
}

// Don't lint in these cases (#64106).
fn or_patterns_no_lint() {
    match Box::new(0) {
        box (0 | 1) => {} // Should not lint as `box 0 | 1` binds as `(box 0) | 1`.
        _ => {}
    }

    match 0 {
        x @ (0 | 1) => {} // Should not lint as `x @ 0 | 1` binds as `(x @ 0) | 1`.
        _ => {}
    }

    if let &(0 | 1) = &0 {} // Should also not lint.
    if let &mut (0 | 1) = &mut 0 {} // Same.

    fn foo((Ok(a) | Err(a)): Result<u8, u8>) {} // Doesn't parse if we remove parens for now.
    //~^ ERROR identifier `a` is bound more than once

    let _ = |(Ok(a) | Err(a)): Result<u8, u8>| 1; // `|Ok(a) | Err(a)| 1` parses as bit-or.
    //~^ ERROR identifier `a` is bound more than once
}

fn or_patterns_will_lint() {
    if let (0 | 1) = 0 {} //~ ERROR unnecessary parentheses around pattern
    if let ((0 | 1),) = (0,) {} //~ ERROR unnecessary parentheses around pattern
    if let [(0 | 1)] = [0] {} //~ ERROR unnecessary parentheses around pattern
    if let 0 | (1 | 2) = 0 {} //~ ERROR unnecessary parentheses around pattern
    struct TS(u8);
    if let TS((0 | 1)) = TS(0) {} //~ ERROR unnecessary parentheses around pattern
    struct NS { f: u8 }
    if let NS { f: (0 | 1) } = (NS { f: 0 }) {} //~ ERROR unnecessary parentheses around pattern
}

// Don't lint on `&(mut x)` because `&mut x` means something else (#55342).
fn deref_mut_binding_no_lint() {
    let &(mut x) = &0;
}

fn main() {
    match 1 {
        (_) => {} //~ ERROR unnecessary parentheses around pattern
        (y) => {} //~ ERROR unnecessary parentheses around pattern
        (ref r) => {} //~ ERROR unnecessary parentheses around pattern
        (e @ 1...2) => {} //~ ERROR unnecessary parentheses around pattern
        (1...2) => {} // Non ambiguous range pattern should not warn
        e @ (3...4) => {} // Non ambiguous range pattern should not warn
    }

    match &1 {
        (e @ &(1...2)) => {} //~ ERROR unnecessary parentheses around pattern
        &(_) => {} //~ ERROR unnecessary parentheses around pattern
        e @ &(1...2) => {} // Ambiguous range pattern should not warn
        &(1...2) => {} // Ambiguous range pattern should not warn
    }

    match &1 {
        e @ &(1...2) | e @ &(3...4) => {} // Complex ambiguous pattern should not warn
        &_ => {}
    }

    match 1 {
        (_) => {} //~ ERROR unnecessary parentheses around pattern
        (y) => {} //~ ERROR unnecessary parentheses around pattern
        (ref r) => {} //~ ERROR unnecessary parentheses around pattern
        (e @ 1..=2) => {} //~ ERROR unnecessary parentheses around pattern
        (1..=2) => {} // Non ambiguous range pattern should not warn
        e @ (3..=4) => {} // Non ambiguous range pattern should not warn
    }

    match &1 {
        (e @ &(1..=2)) => {} //~ ERROR unnecessary parentheses around pattern
        &(_) => {} //~ ERROR unnecessary parentheses around pattern
        e @ &(1..=2) => {} // Ambiguous range pattern should not warn
        &(1..=2) => {} // Ambiguous range pattern should not warn
    }

    match &1 {
        e @ &(1..=2) | e @ &(3..=4) => {} // Complex ambiguous pattern should not warn
        &_ => {}
    }
}
