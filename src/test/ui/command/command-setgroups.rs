// run-pass
// ignore-cloudabi
// ignore-emscripten
// ignore-sgx

#![feature(rustc_private)]
#![feature(setgroups)]

extern crate libc;
use std::process::Command;
use std::os::unix::process::CommandExt;

fn main() {
    let max_ngroups = unsafe { libc::sysconf(libc::_SC_NGROUPS_MAX) };
    let max_ngroups = max_ngroups as u32 + 1;
    let vec: Vec<u32> = (0..max_ngroups).collect();
    let p = Command::new("/bin/id").groups(&vec[..]).spawn();
    assert!(p.is_err());
}
