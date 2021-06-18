use crate::utils::spawn_and_wait;
use crate::utils::try_hard_link;
use crate::SysrootKind;
use std::fs;
use std::path::Path;
use std::process::{self, Command};

pub(crate) fn build_sysroot(
    channel: &str,
    sysroot_kind: SysrootKind,
    target_dir: &Path,
    cg_clif_dylib: String,
    host_triple: &str,
    target_triple: &str,
) {
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).unwrap();
    }
    fs::create_dir_all(target_dir.join("bin")).unwrap();
    fs::create_dir_all(target_dir.join("lib")).unwrap();

    // Copy the backend
    for file in ["cg_clif", "cg_clif_build_sysroot"] {
        try_hard_link(
            Path::new("target").join(channel).join(file),
            target_dir.join("bin").join(file),
        );
    }

    try_hard_link(
        Path::new("target").join(channel).join(&cg_clif_dylib),
        target_dir.join("lib").join(cg_clif_dylib),
    );

    // Copy supporting files
    try_hard_link("rust-toolchain", target_dir.join("rust-toolchain"));
    try_hard_link("scripts/config.sh", target_dir.join("config.sh"));
    try_hard_link("scripts/cargo.sh", target_dir.join("cargo.sh"));

    let default_sysroot = crate::rustc_info::get_default_sysroot();

    let rustlib = target_dir.join("lib").join("rustlib");
    let host_rustlib_lib = rustlib.join(host_triple).join("lib");
    let target_rustlib_lib = rustlib.join(target_triple).join("lib");
    fs::create_dir_all(&host_rustlib_lib).unwrap();
    fs::create_dir_all(&target_rustlib_lib).unwrap();

    if target_triple == "x86_64-pc-windows-gnu" {
        if !default_sysroot.join("lib").join("rustlib").join(target_triple).join("lib").exists() {
            eprintln!(
                "The x86_64-pc-windows-gnu target needs to be installed first before it is possible \
                to compile a sysroot for it.",
            );
            process::exit(1);
        }
        for file in fs::read_dir(
            default_sysroot.join("lib").join("rustlib").join(target_triple).join("lib"),
        )
        .unwrap()
        {
            let file = file.unwrap().path();
            if file.extension().map_or(true, |ext| ext.to_str().unwrap() != "o") {
                continue; // only copy object files
            }
            try_hard_link(&file, target_rustlib_lib.join(file.file_name().unwrap()));
        }
    }

    match sysroot_kind {
        SysrootKind::None => {} // Nothing to do
        SysrootKind::Llvm => {
            for file in fs::read_dir(
                default_sysroot.join("lib").join("rustlib").join(host_triple).join("lib"),
            )
            .unwrap()
            {
                let file = file.unwrap().path();
                let file_name_str = file.file_name().unwrap().to_str().unwrap();
                if file_name_str.contains("rustc_")
                    || file_name_str.contains("chalk")
                    || file_name_str.contains("tracing")
                    || file_name_str.contains("regex")
                {
                    // These are large crates that are part of the rustc-dev component and are not
                    // necessary to run regular programs.
                    continue;
                }
                try_hard_link(&file, host_rustlib_lib.join(file.file_name().unwrap()));
            }

            if target_triple != host_triple {
                for file in fs::read_dir(
                    default_sysroot.join("lib").join("rustlib").join(target_triple).join("lib"),
                )
                .unwrap()
                {
                    let file = file.unwrap().path();
                    try_hard_link(&file, target_rustlib_lib.join(file.file_name().unwrap()));
                }
            }
        }
        SysrootKind::Clif => {
            build_clif_sysroot_for_triple(channel, target_dir, target_triple);

            if host_triple != target_triple {
                build_clif_sysroot_for_triple(channel, target_dir, host_triple);
            }

            // Copy std for the host to the lib dir. This is necessary for the jit mode to find
            // libstd.
            for file in fs::read_dir(host_rustlib_lib).unwrap() {
                let file = file.unwrap().path();
                if file.file_name().unwrap().to_str().unwrap().contains("std-") {
                    try_hard_link(&file, target_dir.join("lib").join(file.file_name().unwrap()));
                }
            }
        }
    }
}

fn build_clif_sysroot_for_triple(channel: &str, target_dir: &Path, triple: &str) {
    let build_dir = Path::new("build_sysroot").join("target").join(triple).join(channel);

    let keep_sysroot =
        fs::read_to_string("config.txt").unwrap().lines().any(|line| line.trim() == "keep_sysroot");
    if !keep_sysroot {
        // Cleanup the target dir with the exception of build scripts and the incremental cache
        for dir in ["build", "deps", "examples", "native"] {
            if build_dir.join(dir).exists() {
                fs::remove_dir_all(build_dir.join(dir)).unwrap();
            }
        }
    }

    // Build sysroot
    let mut build_cmd = Command::new("cargo");
    build_cmd.arg("build").arg("--target").arg(triple).current_dir("build_sysroot");
    let mut rustflags = "--clif -Zforce-unstable-if-unmarked".to_string();
    if channel == "release" {
        build_cmd.arg("--release");
        rustflags.push_str(" -Zmir-opt-level=3");
    }
    build_cmd.env("RUSTFLAGS", rustflags);
    build_cmd
        .env("RUSTC", target_dir.join("bin").join("cg_clif_build_sysroot").canonicalize().unwrap());
    // FIXME Enable incremental again once rust-lang/rust#74946 is fixed
    build_cmd.env("CARGO_INCREMENTAL", "0").env("__CARGO_DEFAULT_LIB_METADATA", "cg_clif");
    spawn_and_wait(build_cmd);

    // Copy all relevant files to the sysroot
    for entry in
        fs::read_dir(Path::new("build_sysroot/target").join(triple).join(channel).join("deps"))
            .unwrap()
    {
        let entry = entry.unwrap();
        if let Some(ext) = entry.path().extension() {
            if ext == "rmeta" || ext == "d" || ext == "dSYM" {
                continue;
            }
        } else {
            continue;
        };
        try_hard_link(
            entry.path(),
            target_dir.join("lib").join("rustlib").join(triple).join("lib").join(entry.file_name()),
        );
    }
}
