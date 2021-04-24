use crate::spec::{SanitizerSet, Target, TargetOptions};

pub fn target() -> Target {
    let mut base = super::linux_gnu_base::opts();
    base.max_atomic_width = Some(128);
    base.supported_sanitizers = SanitizerSet::ADDRESS
        | SanitizerSet::LEAK
        | SanitizerSet::MEMORY
        | SanitizerSet::THREAD
        | SanitizerSet::HWADDRESS;

    Target {
        llvm_target: "aarch64-unknown-linux-gnu".to_string(),
        pointer_width: 64,
        data_layout: "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128".to_string(),
        arch: "aarch64".to_string(),
        options: TargetOptions {
            unsupported_abis: super::arm_base::unsupported_abis(),
            mcount: "\u{1}_mcount".to_string(),
            ..base
        },
    }
}
