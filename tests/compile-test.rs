#![feature(test)]

use compiletest_rs as compiletest;
use libtest::TestDescAndFn;

use std::env::{set_var, var};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn clippy_driver_path() -> PathBuf {
    if let Some(path) = option_env!("CLIPPY_DRIVER_PATH") {
        PathBuf::from(path)
    } else {
        PathBuf::from(concat!("target/", env!("PROFILE"), "/clippy-driver"))
    }
}

fn host_libs() -> PathBuf {
    if let Some(path) = option_env!("HOST_LIBS") {
        PathBuf::from(path)
    } else {
        Path::new("target").join(env!("PROFILE"))
    }
}

fn rustc_test_suite() -> Option<PathBuf> {
    option_env!("RUSTC_TEST_SUITE").map(PathBuf::from)
}

fn rustc_lib_path() -> PathBuf {
    option_env!("RUSTC_LIB_PATH").unwrap().into()
}

fn config(mode: &str, dir: PathBuf) -> compiletest::Config {
    let mut config = compiletest::Config::default();

    let cfg_mode = mode.parse().expect("Invalid mode");
    if let Ok(name) = var::<&str>("TESTNAME") {
        let s: String = name.to_owned();
        config.filter = Some(s)
    }

    if rustc_test_suite().is_some() {
        config.run_lib_path = rustc_lib_path();
        config.compile_lib_path = rustc_lib_path();
    }
    config.target_rustcflags = Some(format!(
        "-L {0} -L {0}/deps -Dwarnings -Zui-testing",
        host_libs().display()
    ));

    config.mode = cfg_mode;
    config.build_base = if rustc_test_suite().is_some() {
        // we don't need access to the stderr files on travis
        let mut path = PathBuf::from(env!("OUT_DIR"));
        path.push("test_build_base");
        path
    } else {
        let mut path = std::env::current_dir().unwrap();
        path.push("target/debug/test_build_base");
        path
    };
    config.src_base = dir;
    config.rustc_path = clippy_driver_path();
    config
}

fn run_mode(mode: &str, dir: PathBuf) {
    let cfg = config(mode, dir);
    // clean rmeta data, otherwise "cargo check; cargo test" fails (#2896)
    cfg.clean_rmeta();
    compiletest::run_tests(&cfg);
}

#[warn(clippy::identity_conversion)]
fn run_ui_toml_tests(config: &compiletest::Config, mut tests: Vec<TestDescAndFn>) -> Result<bool, io::Error> {
    let mut result = true;
    let opts = compiletest::test_opts(config);
    for dir in fs::read_dir(&config.src_base)? {
        let dir_path = dir.unwrap().path();
        set_var("CARGO_MANIFEST_DIR", &dir_path);
        for file in fs::read_dir(&dir_path)? {
            let file = file?;
            let file_path = file.path();
            if !file.file_type()?.is_file() {
                continue;
            }
            if file_path.extension() != Some(OsStr::new("rs")) {
                continue;
            }
            let paths = compiletest::common::TestPaths {
                file: file_path,
                base: config.src_base.clone(),
                relative_dir: dir_path.file_name().unwrap().into(),
            };
            let test_name = compiletest::make_test_name(&config, &paths);
            let index = tests
                .iter()
                .position(|test| test.desc.name.to_string() == test_name.to_string())
                .expect("The test should be in there");
            let opts = libtest::TestOpts {
                list: opts.list,
                filter: opts.filter.clone(),
                filter_exact: opts.filter_exact,
                exclude_should_panic: Default::default(),
                run_ignored: libtest::RunIgnored::No,
                run_tests: opts.run_tests,
                bench_benchmarks: opts.bench_benchmarks,
                logfile: opts.logfile.clone(),
                nocapture: opts.nocapture,
                color: libtest::ColorConfig::AutoColor,
                format: libtest::OutputFormat::Pretty,
                test_threads: opts.test_threads,
                skip: opts.skip.clone(),
                options: libtest::Options::new(),
            };
            result &= libtest::run_tests_console(&opts, vec![tests.swap_remove(index)])?;
        }
    }
    Ok(result)
}

fn run_ui_toml() {
    let path = PathBuf::from("tests/ui-toml").canonicalize().unwrap();
    let config = config("ui", path);
    let tests = compiletest::make_tests(&config);

    let tests = tests
        .into_iter()
        .map(|test| {
            libtest::TestDescAndFn {
                desc: libtest::TestDesc {
                    name: libtest::TestName::DynTestName(test.desc.name.to_string()),
                    ignore: test.desc.ignore,
                    allow_fail: test.desc.allow_fail,
                    should_panic: libtest::ShouldPanic::No,
                },
                // oli obk giving up
                testfn: unsafe { std::mem::transmute(test.testfn) },
            }
        })
        .collect();

    let res = run_ui_toml_tests(&config, tests);
    match res {
        Ok(true) => {},
        Ok(false) => panic!("Some tests failed"),
        Err(e) => {
            println!("I/O failure during tests: {:?}", e);
        },
    }
}

fn prepare_env() {
    set_var("CLIPPY_DISABLE_DOCS_LINKS", "true");
    set_var("CLIPPY_TESTS", "true");
    //set_var("RUST_BACKTRACE", "0");
}

#[test]
fn compile_test() {
    prepare_env();
    run_mode("ui", "tests/ui".into());
    run_ui_toml();
}
