#[inline(never)]
fn noop() {}

// EMIT_MIR simplify_match.main.ConstProp.diff
fn main() {
    match { let x = false; x } {
        true => noop(),
        false => {},
    }
}
