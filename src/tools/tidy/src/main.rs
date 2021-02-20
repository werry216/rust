//! Tidy checks source code in this repository.
//!
//! This program runs all of the various tidy checks for style, cleanliness,
//! etc. This is run by default on `./x.py test` and as part of the auto
//! builders. The tidy checks can be executed with `./x.py test tidy`.

use tidy::*;

use crossbeam_utils::thread::scope;
use std::env;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    let root_path: PathBuf = env::args_os().nth(1).expect("need path to root of repo").into();
    let cargo: PathBuf = env::args_os().nth(2).expect("need path to cargo").into();
    let output_directory: PathBuf =
        env::args_os().nth(3).expect("need path to output directory").into();

    let src_path = root_path.join("src");
    let library_path = root_path.join("library");
    let compiler_path = root_path.join("compiler");

    let args: Vec<String> = env::args().skip(1).collect();

    let verbose = args.iter().any(|s| *s == "--verbose");

    let bad = std::sync::Arc::new(AtomicBool::new(false));

    scope(|s| {
        macro_rules! check {
            ($p:ident $(, $args:expr)* ) => {
                s.spawn(|_| {
                    let mut flag = false;
                    $p::check($($args),* , &mut flag);
                    if (flag) {
                        bad.store(true, Ordering::Relaxed);
                    }
                });
            }
        }

        // Checks that are done on the cargo workspace.
        check!(deps, &root_path, &cargo);
        check!(extdeps, &root_path);

        // Checks over tests.
        check!(debug_artifacts, &src_path);
        check!(ui_tests, &src_path);

        // Checks that only make sense for the compiler.
        check!(errors, &compiler_path);
        check!(error_codes_check, &src_path);

        // Checks that only make sense for the std libs.
        check!(pal, &library_path);

        // Checks that need to be done for both the compiler and std libraries.
        check!(unit_tests, &src_path);
        check!(unit_tests, &compiler_path);
        check!(unit_tests, &library_path);

        check!(bins, &src_path, &output_directory);
        check!(bins, &compiler_path, &output_directory);
        check!(bins, &library_path, &output_directory);

        check!(style, &src_path);
        check!(style, &compiler_path);
        check!(style, &library_path);

        check!(edition, &src_path);
        check!(edition, &compiler_path);
        check!(edition, &library_path);

        let collected = {
            let mut flag = false;
            let r = features::check(&src_path, &compiler_path, &library_path, &mut flag, verbose);
            if flag {
                bad.store(true, Ordering::Relaxed);
            }
            r
        };
        check!(unstable_book, &src_path, collected);
    })
    .unwrap();

    if bad.load(Ordering::Relaxed) {
        eprintln!("some tidy checks failed");
        process::exit(1);
    }
}
