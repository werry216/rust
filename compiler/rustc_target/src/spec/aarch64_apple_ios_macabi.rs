use super::apple_sdk_base::{opts, Arch};
use crate::spec::{Target, TargetOptions};

pub fn target() -> Target {
    let base = opts("ios", Arch::Arm64_macabi);
    Target {
        llvm_target: "arm64-apple-ios14.0-macabi".to_string(),
        pointer_width: 64,
        data_layout: "e-m:o-i64:64-i128:128-n32:64-S128".to_string(),
        arch: "aarch64".to_string(),
        options: TargetOptions {
            features: "+neon,+fp-armv8,+apple-a12".to_string(),
            max_atomic_width: Some(128),
            unsupported_abis: super::arm_base::unsupported_abis(),
            forces_embed_bitcode: true,
            // Taken from a clang build on Xcode 11.4.1.
            // These arguments are not actually invoked - they just have
            // to look right to pass App Store validation.
            bitcode_llvm_cmdline: "-triple\0\
                arm64-apple-ios14.0-macabi\0\
                -emit-obj\0\
                -disable-llvm-passes\0\
                -Os\0"
                .to_string(),
            ..base
        },
    }
}
