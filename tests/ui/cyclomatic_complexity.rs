// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




#![allow(clippy::all)]
#![warn(clippy::cyclomatic_complexity)]
#![allow(unused)]

fn main() {
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
    if true {
        println!("a");
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn kaboom() {
    let n = 0;
    'a: for i in 0..20 {
        'b: for j in i..20 {
            for k in j..20 {
                if k == 5 {
                    break 'b;
                }
                if j == 3 && k == 6 {
                    continue 'a;
                }
                if k == j {
                    continue;
                }
                println!("bake");
            }
        }
        println!("cake");
    }
}

fn bloo() {
    match 42 {
        0 => println!("hi"),
        1 => println!("hai"),
        2 => println!("hey"),
        3 => println!("hallo"),
        4 => println!("hello"),
        5 => println!("salut"),
        6 => println!("good morning"),
        7 => println!("good evening"),
        8 => println!("good afternoon"),
        9 => println!("good night"),
        10 => println!("bonjour"),
        11 => println!("hej"),
        12 => println!("hej hej"),
        13 => println!("greetings earthling"),
        14 => println!("take us to you leader"),
        15 | 17 | 19 | 21 | 23 | 25 | 27 | 29 | 31 | 33 => println!("take us to you leader"),
        35 | 37 | 39 | 41 | 43 | 45 | 47 | 49 | 51 | 53 => println!("there is no undefined behavior"),
        55 | 57 | 59 | 61 | 63 | 65 | 67 | 69 | 71 | 73 => println!("I know borrow-fu"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn lots_of_short_circuits() -> bool {
    true && false && true && false && true && false && true
}

#[clippy::cyclomatic_complexity = "0"]
fn lots_of_short_circuits2() -> bool {
    true || false || true || false || true || false || true
}

#[clippy::cyclomatic_complexity = "0"]
fn baa() {
    let x = || match 99 {
        0 => 0,
        1 => 1,
        2 => 2,
        4 => 4,
        6 => 6,
        9 => 9,
        _ => 42,
    };
    if x() == 42 {
        println!("x");
    } else {
        println!("not x");
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn bar() {
    match 99 {
        0 => println!("hi"),
        _ => println!("bye"),
    }
}

#[test]
#[clippy::cyclomatic_complexity = "0"]
/// Tests are usually complex but simple at the same time. `clippy::cyclomatic_complexity` used to give
/// lots of false-positives in tests.
fn dont_warn_on_tests() {
    match 99 {
        0 => println!("hi"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barr() {
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barr2() {
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barrr() {
    match 99 {
        0 => println!("hi"),
        1 => panic!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barrr2() {
    match 99 {
        0 => println!("hi"),
        1 => panic!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
    match 99 {
        0 => println!("hi"),
        1 => panic!("bla"),
        2 | 3 => println!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barrrr() {
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => panic!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn barrrr2() {
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => panic!("blub"),
        _ => println!("bye"),
    }
    match 99 {
        0 => println!("hi"),
        1 => println!("bla"),
        2 | 3 => panic!("blub"),
        _ => println!("bye"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn cake() {
    if 4 == 5 {
        println!("yea");
    } else {
        panic!("meh");
    }
    println!("whee");
}


#[clippy::cyclomatic_complexity = "0"]
pub fn read_file(input_path: &str) -> String {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::Path;
    let mut file = match File::open(&Path::new(input_path)) {
        Ok(f) => f,
        Err(err) => {
            panic!("Can't open {}: {}", input_path, err);
        }
    };

    let mut bytes = Vec::new();

    match file.read_to_end(&mut bytes) {
        Ok(..) => {},
        Err(_) => {
            panic!("Can't read {}", input_path);
        }
    };

    match String::from_utf8(bytes) {
        Ok(contents) => contents,
        Err(_) => {
            panic!("{} is not UTF-8 encoded", input_path);
        }
    }
}

enum Void {}

#[clippy::cyclomatic_complexity = "0"]
fn void(void: Void) {
    if true {
        match void {
        }
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn mcarton_sees_all() {
    panic!("meh");
    panic!("möh");
}

#[clippy::cyclomatic_complexity = "0"]
fn try() -> Result<i32, &'static str> {
    match 5 {
        5 => Ok(5),
        _ => return Err("bla"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn try_again() -> Result<i32, &'static str> {
    let _ = try!(Ok(42));
    let _ = try!(Ok(43));
    let _ = try!(Ok(44));
    let _ = try!(Ok(45));
    let _ = try!(Ok(46));
    let _ = try!(Ok(47));
    let _ = try!(Ok(48));
    let _ = try!(Ok(49));
    match 5 {
        5 => Ok(5),
        _ => return Err("bla"),
    }
}

#[clippy::cyclomatic_complexity = "0"]
fn early() -> Result<i32, &'static str> {
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
    return Ok(5);
}

#[clippy::cyclomatic_complexity = "0"]
fn early_ret() -> i32 {
    let a = if true { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    let a = if a < 99 { 42 } else { return 0; };
    match 5 {
        5 => 5,
        _ => return 6,
    }
}
