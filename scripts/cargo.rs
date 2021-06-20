use std::env;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    if env::var("RUSTC_WRAPPER").map_or(false, |wrapper| wrapper.contains("sccache")) {
        eprintln!(
            "\x1b[1;93m=== Warning: Unsetting RUSTC_WRAPPER to prevent interference with sccache ===\x1b[0m"
        );
        env::remove_var("RUSTC_WRAPPER");
    }

    let sysroot = PathBuf::from(env::current_exe().unwrap().parent().unwrap());

    env::set_var("RUSTC", sysroot.join("bin/cg_clif".to_string() + env::consts::EXE_SUFFIX));

    let mut rustdoc_flags = env::var("RUSTDOCFLAGS").unwrap_or(String::new());
    rustdoc_flags.push_str(" -Cpanic=abort -Zpanic-abort-tests -Zcodegen-backend=");
    rustdoc_flags.push_str(
        sysroot
            .join(if cfg!(windows) { "bin" } else { "lib" })
            .join(
                env::consts::DLL_PREFIX.to_string()
                    + "rustc_codegen_cranelift"
                    + env::consts::DLL_SUFFIX,
            )
            .to_str()
            .unwrap(),
    );
    rustdoc_flags.push_str(" --sysroot ");
    rustdoc_flags.push_str(sysroot.to_str().unwrap());
    env::set_var("RUSTDOCFLAGS", rustdoc_flags);

    let default_sysroot = Command::new("rustc")
        .stderr(Stdio::inherit())
        .args(&["--print", "sysroot"])
        .output()
        .unwrap()
        .stdout;
    let default_sysroot = std::str::from_utf8(&default_sysroot).unwrap().trim();

    let extra_ld_lib_path =
        default_sysroot.to_string() + ":" + sysroot.join("lib").to_str().unwrap();
    if cfg!(target_os = "macos") {
        env::set_var(
            "DYLD_LIBRARY_PATH",
            env::var("DYLD_LIBRARY_PATH").unwrap_or(String::new()) + ":" + &extra_ld_lib_path,
        );
    } else if cfg!(unix) {
        env::set_var(
            "LD_LIBRARY_PATH",
            env::var("LD_LIBRARY_PATH").unwrap_or(String::new()) + ":" + &extra_ld_lib_path,
        );
    }

    // Ensure that the right toolchain is used
    env::set_var("RUSTUP_TOOLCHAIN", env!("RUSTUP_TOOLCHAIN"));

    let args: Vec<_> = match env::args().nth(1).as_deref() {
        Some("jit") => {
            env::set_var(
                "RUSTFLAGS",
                env::var("RUSTFLAGS").unwrap_or(String::new()) + " -Cprefer-dynamic",
            );
            std::array::IntoIter::new(["rustc".to_string()])
                .chain(env::args().skip(2))
                .chain(["--".to_string(), "-Cllvm-args=mode=jit".to_string()])
                .collect()
        }
        Some("lazy-jit") => {
            env::set_var(
                "RUSTFLAGS",
                env::var("RUSTFLAGS").unwrap_or(String::new()) + " -Cprefer-dynamic",
            );
            std::array::IntoIter::new(["rustc".to_string()])
                .chain(env::args().skip(2))
                .chain(["--".to_string(), "-Cllvm-args=mode=jit-lazy".to_string()])
                .collect()
        }
        _ => env::args().skip(1).collect(),
    };

    Command::new("cargo").args(args).exec();
}
