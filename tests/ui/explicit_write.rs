// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




#![warn(clippy::explicit_write)]


fn stdout() -> String {
    String::new()
}

fn stderr() -> String {
    String::new()
}

fn main() {
    // these should warn
    {
        use std::io::Write;
        write!(std::io::stdout(), "test").unwrap();
        write!(std::io::stderr(), "test").unwrap();
        writeln!(std::io::stdout(), "test").unwrap();
        writeln!(std::io::stderr(), "test").unwrap();
        std::io::stdout().write_fmt(format_args!("test")).unwrap();
        std::io::stderr().write_fmt(format_args!("test")).unwrap();
    }
    // these should not warn, different destination
    {
        use std::fmt::Write;
        let mut s = String::new();
        write!(s, "test").unwrap();
        write!(s, "test").unwrap();
        writeln!(s, "test").unwrap();
        writeln!(s, "test").unwrap();
        s.write_fmt(format_args!("test")).unwrap();
        s.write_fmt(format_args!("test")).unwrap();
        write!(stdout(), "test").unwrap();
        write!(stderr(), "test").unwrap();
        writeln!(stdout(), "test").unwrap();
        writeln!(stderr(), "test").unwrap();
        stdout().write_fmt(format_args!("test")).unwrap();
        stderr().write_fmt(format_args!("test")).unwrap();
    }
    // these should not warn, no unwrap
    {
        use std::io::Write;
        std::io::stdout().write_fmt(format_args!("test")).expect("no stdout");
        std::io::stderr().write_fmt(format_args!("test")).expect("no stderr");
    }
}
