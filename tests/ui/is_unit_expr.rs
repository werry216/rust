

#![warn(unit_expr)]
#[allow(unused_variables)]

fn main() {
    // lint should note removing the semicolon from "baz"
    let x = {
        "foo";
        "baz";
    };


    // lint should ignore false positive.
    let y = if true {
        "foo"
    } else {
        return;
    };

    // lint should note removing semicolon from "bar"
    let z = if true {
        "foo";
    } else {
        "bar";
    };


    let a1 = Some(5);

    // lint should ignore false positive
    let a2 = match a1 {
        Some(x) => x,
        _ => {
            return;
        },
    };

    // lint should note removing the semicolon after `x;`
    let a3 = match a1 {
        Some(x) => {
            x;
        },
        _ => {
            0;
        },
    };
    
    loop {
        let a2 = match a1 {
            Some(x) => x,
            _ => {
                break;
            },
        };
        let a2 = match a1 {
            Some(x) => x,
            _ => {
                continue;
            },
        };
    }
}

pub fn foo() -> i32 {
    let a2 = match None {
        Some(x) => x,
        _ => {
            return 42;
        },
    };
    55
}

pub fn issue_2160() {
    let x1 = {};
    let x2 = if true {} else {};
    let x3 = match None { Some(_) => {}, None => {}, };
}
