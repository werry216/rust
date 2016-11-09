// error-pattern:yummy
#![feature(box_syntax)]
#![feature(rustc_private)]
#![feature(static_in_const)]

#![allow(unknown_lints, missing_docs_in_private_items)]

extern crate clippy_lints;
extern crate getopts;
extern crate rustc;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_plugin;
extern crate syntax;

use rustc_driver::{driver, CompilerCalls, RustcDefaultCalls, Compilation};
use rustc::session::{config, Session};
use rustc::session::config::{Input, ErrorOutputType};
use std::path::PathBuf;
use std::process::{self, Command};
use syntax::ast;
use std::io::{self, Write};

use clippy_lints::utils::cargo;

struct ClippyCompilerCalls {
    default: RustcDefaultCalls,
    run_lints: bool,
}

impl ClippyCompilerCalls {
    fn new(run_lints: bool) -> Self {
        ClippyCompilerCalls {
            default: RustcDefaultCalls,
            run_lints: run_lints,
        }
    }
}

impl<'a> CompilerCalls<'a> for ClippyCompilerCalls {
    fn early_callback(&mut self,
                      matches: &getopts::Matches,
                      sopts: &config::Options,
                      cfg: &ast::CrateConfig,
                      descriptions: &rustc_errors::registry::Registry,
                      output: ErrorOutputType)
                      -> Compilation {
        self.default.early_callback(matches, sopts, cfg, descriptions, output)
    }
    fn no_input(&mut self,
                matches: &getopts::Matches,
                sopts: &config::Options,
                cfg: &ast::CrateConfig,
                odir: &Option<PathBuf>,
                ofile: &Option<PathBuf>,
                descriptions: &rustc_errors::registry::Registry)
                -> Option<(Input, Option<PathBuf>)> {
        self.default.no_input(matches, sopts, cfg, odir, ofile, descriptions)
    }
    fn late_callback(&mut self,
                     matches: &getopts::Matches,
                     sess: &Session,
                     input: &Input,
                     odir: &Option<PathBuf>,
                     ofile: &Option<PathBuf>)
                     -> Compilation {
        self.default.late_callback(matches, sess, input, odir, ofile)
    }
    fn build_controller(&mut self, sess: &Session, matches: &getopts::Matches) -> driver::CompileController<'a> {
        let mut control = self.default.build_controller(sess, matches);

        if self.run_lints {
            let old = std::mem::replace(&mut control.after_parse.callback, box |_| {});
            control.after_parse.callback = Box::new(move |state| {
                {
                    let mut registry = rustc_plugin::registry::Registry::new(state.session, state.krate.as_ref().expect("at this compilation stage the krate must be parsed").span);
                    registry.args_hidden = Some(Vec::new());
                    clippy_lints::register_plugins(&mut registry);

                    let rustc_plugin::registry::Registry { early_lint_passes,
                                                           late_lint_passes,
                                                           lint_groups,
                                                           llvm_passes,
                                                           attributes,
                                                           mir_passes,
                                                           .. } = registry;
                    let sess = &state.session;
                    let mut ls = sess.lint_store.borrow_mut();
                    for pass in early_lint_passes {
                        ls.register_early_pass(Some(sess), true, pass);
                    }
                    for pass in late_lint_passes {
                        ls.register_late_pass(Some(sess), true, pass);
                    }

                    for (name, to) in lint_groups {
                        ls.register_group(Some(sess), true, name, to);
                    }

                    sess.plugin_llvm_passes.borrow_mut().extend(llvm_passes);
                    sess.mir_passes.borrow_mut().extend(mir_passes);
                    sess.plugin_attributes.borrow_mut().extend(attributes);
                }
                old(state);
            });
        }

        control
    }
}

use std::path::Path;

const CARGO_CLIPPY_HELP: &str = r#"Checks a package to catch common mistakes and improve your Rust code.

Usage:
    cargo clippy [options] [--] [<opts>...]

Common options:
    -h, --help               Print this message
    --features               Features to compile for the package
    -V, --version            Print version info and exit

Other options are the same as `cargo rustc`.

To allow or deny a lint from the command line you can use `cargo clippy --`
with:

    -W --warn OPT       Set lint warnings
    -A --allow OPT      Set lint allowed
    -D --deny OPT       Set lint denied
    -F --forbid OPT     Set lint forbidden

The feature `cargo-clippy` is automatically defined for convenience. You can use
it to allow or deny lints from the code, eg.:

    #[cfg_attr(feature = "cargo-clippy", allow(needless_lifetimes))]
"#;

#[allow(print_stdout)]
fn show_help() {
    println!("{}", CARGO_CLIPPY_HELP);
}

#[allow(print_stdout)]
fn show_version() {
    println!("{}", env!("CARGO_PKG_VERSION"));
}

pub fn main() {
    use std::env;

    if env::var("CLIPPY_DOGFOOD").map(|_| true).unwrap_or(false) {
        panic!("yummy");
    }
    
    // Check for version and help flags even when invoked as 'cargo-clippy'
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        show_help();
        return;
    }
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        show_version();
        return;
    }

    let dep_path = env::current_dir().expect("current dir is not readable").join("target").join("debug").join("deps");

    if let Some("clippy") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        // this arm is executed on the initial call to `cargo clippy`

        let manifest_path_arg = std::env::args().skip(2).find(|val| val.starts_with("--manifest-path="));

        let mut metadata = if let Ok(metadata) = cargo::metadata(manifest_path_arg.as_ref().map(AsRef::as_ref)) {
            metadata
        } else {
            let _ = io::stderr().write_fmt(format_args!("error: Could not obtain cargo metadata."));
            process::exit(101);
        };

        assert_eq!(metadata.version, 1);

        let manifest_path = manifest_path_arg.map(|arg| PathBuf::from(Path::new(&arg["--manifest-path=".len()..])));

        let current_dir = std::env::current_dir();

        let package_index = metadata.packages.iter()
            .position(|package| {
                let package_manifest_path = Path::new(&package.manifest_path);
                if let Some(ref manifest_path) = manifest_path {
                    package_manifest_path == manifest_path
                } else {
                    let current_dir = current_dir.as_ref().expect("could not read current directory");
                    let package_manifest_directory = package_manifest_path.parent().expect("could not find parent directory of package manifest");
                    package_manifest_directory == current_dir
                }
            })
            .expect("could not find matching package");
        let package = metadata.packages.remove(package_index);
        for target in package.targets {
            let args = std::env::args().skip(2);
            if let Some(first) = target.kind.get(0) {
                if target.kind.len() > 1 || first.ends_with("lib") {
                    if let Err(code) = process(std::iter::once("--lib".to_owned()).chain(args), &dep_path) {
                        std::process::exit(code);
                    }
                } else if ["bin", "example", "test", "bench"].contains(&&**first) {
                    if let Err(code) = process(vec![format!("--{}", first), target.name].into_iter().chain(args), &dep_path) {
                        std::process::exit(code);
                    }
                }
            } else {
                panic!("badly formatted cargo metadata: target::kind is an empty array");
            }
        }
    } else {
        // this arm is executed when cargo-clippy runs `cargo rustc` with the `RUSTC` env var set to itself

        let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
        let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
        let sys_root = if let (Some(home), Some(toolchain)) = (home, toolchain) {
            format!("{}/toolchains/{}", home, toolchain)
        } else {
            option_env!("SYSROOT")
                .map(|s| s.to_owned())
                .or(Command::new("rustc")
                    .arg("--print")
                    .arg("sysroot")
                    .output()
                    .ok()
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .map(|s| s.trim().to_owned()))
                .expect("need to specify SYSROOT env var during clippy compilation, or use rustup or multirust")
        };

        // this conditional check for the --sysroot flag is there so users can call `cargo-clippy` directly
        // without having to pass --sysroot or anything
        let mut args: Vec<String> = if env::args().any(|s| s == "--sysroot") {
            env::args().collect()
        } else {
            env::args().chain(Some("--sysroot".to_owned())).chain(Some(sys_root)).collect()
        };

        // this check ensures that dependencies are built but not linted and the final crate is
        // linted but not built
        let clippy_enabled = env::args().any(|s| s == "-Zno-trans");

        if clippy_enabled {
            args.extend_from_slice(&["--cfg".to_owned(), r#"feature="cargo-clippy""#.to_owned()]);
        }

        let mut ccc = ClippyCompilerCalls::new(clippy_enabled);
        let (result, _) = rustc_driver::run_compiler(&args, &mut ccc, None, None);

        if let Err(err_count) = result {
            if err_count > 0 {
                std::process::exit(1);
            }
        }
    }
}

fn process<P, I>(old_args: I, dep_path: P) -> Result<(), i32>
    where P: AsRef<Path>,
          I: Iterator<Item = String>
{

    let mut args = vec!["rustc".to_owned()];

    let mut found_dashes = false;
    for arg in old_args {
        found_dashes |= arg == "--";
        args.push(arg);
    }
    if !found_dashes {
        args.push("--".to_owned());
    }
    args.push("-L".to_owned());
    args.push(dep_path.as_ref().to_string_lossy().into_owned());
    args.push("-Zno-trans".to_owned());
    args.push("--cfg".to_owned());
    args.push(r#"feature="cargo-clippy""#.to_owned());

    let path = std::env::current_exe().expect("current executable path invalid");
    let exit_status = std::process::Command::new("cargo")
        .args(&args)
        .env("RUSTC", path)
        .spawn().expect("could not run cargo")
        .wait().expect("failed to wait for cargo?");

    if exit_status.success() {
        Ok(())
    } else {
        Err(exit_status.code().unwrap_or(-1))
    }
}
