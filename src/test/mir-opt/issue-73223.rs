fn main() {
    let split = match Some(1) {
        Some(v) => v,
        None => return,
    };

    let _prev = Some(split);
    assert_eq!(split, 1);
}

// EMIT_MIR_FOR_EACH_BIT_WIDTH
// EMIT_MIR rustc.main.SimplifyArmIdentity.diff
// EMIT_MIR rustc.main.PreCodegen.diff
