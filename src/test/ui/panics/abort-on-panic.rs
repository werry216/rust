// run-pass

#![allow(unused_must_use)]
#![feature(unwind_attributes)]
// Since we mark some ABIs as "nounwind" to LLVM, we must make sure that
// we never unwind through them.

// ignore-emscripten no processes
// ignore-sgx no processes

use std::{env, panic};
use std::io::prelude::*;
use std::io;
use std::process::{Command, Stdio};

#[unwind(aborts)] // FIXME(#58794) should work even without the attribute
extern "C" fn panic_in_ffi() {
    panic!("Test");
}

#[unwind(aborts)]
extern "Rust" fn panic_in_rust_abi() {
    panic!("TestRust");
}

fn should_have_aborted() {
    io::stdout().write(b"This should never be printed.\n");
    let _ = io::stdout().flush();
}

fn bomb_out_but_not_abort(msg: &str) {
    eprintln!("bombing out: {}", msg);
    exit(1);
}

fn test() {
    let _ = panic::catch_unwind(|| { panic_in_ffi(); });
    should_have_aborted();
}

fn testrust() {
    let _ = panic::catch_unwind(|| { panic_in_rust_abi(); });
    should_have_aborted();
}

fn test_always_abort() {
    panic::always_abort();
    let _ = panic::catch_unwind(|| { panic!(); });
    should_have_aborted();
}

fn main() {
    let tests: &[(_, fn())] = &[
        ("test", test),
        ("testrust", testrust),
        ("test_always_abort", test_always_abort),
    ];

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        // This is inside the self-executed command.
        for (a,f) in tests {
            if &args[1] == a { return f() }
        }
        bomb_out_but_not_abort("bad test");
    }

    let execute_self_expecting_abort = |arg| {
        let mut p = Command::new(&args[0])
                            .stdout(Stdio::piped())
                            .stdin(Stdio::piped())
                            .arg(arg).spawn().unwrap();
        let status = p.wait().unwrap();
        assert!(!status.success());
        // Any reasonable platform can distinguish a process which
        // called exit(1) from one which panicked.
        assert_ne!(status.code(), Some(1));
    };

    for (a,_f) in tests {
        execute_self_expecting_abort(a);
    }
}
