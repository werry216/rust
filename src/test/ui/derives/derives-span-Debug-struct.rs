// ignore-x86 FIXME: missing sysroot spans (#53081)
// This file was auto-generated using 'src/etc/generate-deriving-span-tests.py'


struct Error;

#[derive(Debug)]
struct Struct {
    x: Error //~ ERROR
}

fn main() {}
