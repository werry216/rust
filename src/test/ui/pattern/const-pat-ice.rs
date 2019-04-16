// failure-status: 101
// rustc-env:RUST_BACKTRACE=0
// normalize-stderr-test "note: rustc 1.* running on .*" -> "note: rustc VERSION running on TARGET"
// normalize-stderr-test "note: compiler flags: .*" -> "note: compiler flags: FLAGS"

// This is a repro test for an ICE in our pattern handling of constants.

const FOO: &&&u32 = &&&42;

fn main() {
    match unimplemented!() {
        &&&42 => {},
        FOO => {},
        _ => {},
    }
}
