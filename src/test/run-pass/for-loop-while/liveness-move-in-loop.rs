// run-pass
#![allow(dead_code)]

// pretty-expanded FIXME #23616

fn take(x: isize) -> isize {x}

fn the_loop() {
    let mut list = Vec::new();
    loop {
        let x = 5;
        if x > 3 {
            list.push(take(x));
        } else {
            break;
        }
    }
}

pub fn main() {}
