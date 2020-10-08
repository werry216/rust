use crate::spec::{LinkerFlavor, Target, TargetOptions};

pub fn target() -> Target {
    Target {
        llvm_target: "mips64el-unknown-linux-gnuabi64".to_string(),
        pointer_width: 64,
        data_layout: "e-m:e-i8:8:32-i16:16:32-i64:64-n32:64-S128".to_string(),
        arch: "mips64".to_string(),
        target_os: "linux".to_string(),
        target_env: "gnu".to_string(),
        target_vendor: "unknown".to_string(),
        linker_flavor: LinkerFlavor::Gcc,
        options: TargetOptions {
            // NOTE(mips64r2) matches C toolchain
            cpu: "mips64r2".to_string(),
            features: "+mips64r2".to_string(),
            max_atomic_width: Some(64),
            target_mcount: "_mcount".to_string(),

            ..super::linux_base::opts()
        },
    }
}
