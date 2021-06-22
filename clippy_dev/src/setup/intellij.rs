use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

// This module takes an absolute path to a rustc repo and alters the dependencies to point towards
// the respective rustc subcrates instead of using extern crate xyz.
// This allows rust analyzer to analyze rustc internals and show proper information inside clippy
// code. See https://github.com/rust-analyzer/rust-analyzer/issues/3517 and https://github.com/rust-lang/rust-clippy/issues/5514 for details

const RUSTC_PATH_SECTION: &str = "[target.'cfg(NOT_A_PLATFORM)'.dependencies]";
const DEPENDENCIES_SECTION: &str = "[dependencies]";

const CLIPPY_PROJECTS: &[ClippyProjectInfo] = &[
    ClippyProjectInfo::new("root", "Cargo.toml", "src/driver.rs"),
    ClippyProjectInfo::new("clippy_lints", "clippy_lints/Cargo.toml", "clippy_lints/src/lib.rs"),
    ClippyProjectInfo::new("clippy_utils", "clippy_utils/Cargo.toml", "clippy_utils/src/lib.rs"),
];

/// Used to store clippy project information to later inject the dependency into.
struct ClippyProjectInfo {
    /// Only used to display information to the user
    name: &'static str,
    cargo_file: &'static str,
    lib_rs_file: &'static str,
}

impl ClippyProjectInfo {
    const fn new(name: &'static str, cargo_file: &'static str, lib_rs_file: &'static str) -> Self {
        Self {
            name,
            cargo_file,
            lib_rs_file,
        }
    }
}

pub fn setup_rustc_src(rustc_path: &str) {
    let rustc_source_dir = match check_and_get_rustc_dir(rustc_path) {
        Ok(path) => path,
        Err(_) => return,
    };

    for project in CLIPPY_PROJECTS {
        if inject_deps_into_project(&rustc_source_dir, project).is_err() {
            return;
        }
    }

    println!("info: the source paths can be removed again with `cargo dev remove intellij`");
}

fn check_and_get_rustc_dir(rustc_path: &str) -> Result<PathBuf, ()> {
    let mut path = PathBuf::from(rustc_path);

    if path.is_relative() {
        match path.canonicalize() {
            Ok(absolute_path) => {
                println!("info: the rustc path was resolved to: `{}`", absolute_path.display());
                path = absolute_path;
            },
            Err(err) => {
                eprintln!("error: unable to get the absolute path of rustc ({})", err);
                return Err(());
            },
        };
    }

    let path = path.join("compiler");
    println!("info: looking for compiler sources at: {}", path.display());

    if !path.exists() {
        eprintln!("error: the given path does not exist");
        return Err(());
    }

    if !path.is_dir() {
        eprintln!("error: the given path is a file and not a directory");
        return Err(());
    }

    Ok(path)
}

fn inject_deps_into_project(rustc_source_dir: &Path, project: &ClippyProjectInfo) -> Result<(), ()> {
    let cargo_content = read_project_file(project.cargo_file, "Cargo.toml", project.name)?;
    let lib_content = read_project_file(project.lib_rs_file, "lib.rs", project.name)?;

    if inject_deps_into_manifest(rustc_source_dir, project.cargo_file, &cargo_content, &lib_content).is_err() {
        eprintln!(
            "error: unable to inject dependencies into {} with the Cargo file {}",
            project.name, project.cargo_file
        );
        Err(())
    } else {
        Ok(())
    }
}

/// `clippy_dev` expects to be executed in the root directory of Clippy. This function
/// loads the given file or returns an error. Having it in this extra function ensures
/// that the error message looks nice.
fn read_project_file(file_path: &str, file_name: &str, project: &str) -> Result<String, ()> {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!(
            "error: unable to find the `{}` file for the project {}",
            file_name, project
        );
        return Err(());
    }

    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) => {
            println!(
                "error: the `{}` file for the project {} could not be read ({})",
                file_name, project, err
            );
            Err(())
        },
    }
}

fn inject_deps_into_manifest(
    rustc_source_dir: &Path,
    manifest_path: &str,
    cargo_toml: &str,
    lib_rs: &str,
) -> std::io::Result<()> {
    // do not inject deps if we have already done so
    if cargo_toml.contains(RUSTC_PATH_SECTION) {
        eprintln!(
            "warn: dependencies are already setup inside {}, skipping file",
            manifest_path
        );
        return Ok(());
    }

    let extern_crates = lib_rs
        .lines()
        // only take dependencies starting with `rustc_`
        .filter(|line| line.starts_with("extern crate rustc_"))
        // we have something like "extern crate foo;", we only care about the "foo"
        //              ↓          ↓
        // extern crate rustc_middle;
        .map(|s| &s[13..(s.len() - 1)]);

    let new_deps = extern_crates.map(|dep| {
        // format the dependencies that are going to be put inside the Cargo.toml
        format!(
            "{dep} = {{ path = \"{source_path}/{dep}\" }}\n",
            dep = dep,
            source_path = rustc_source_dir.display()
        )
    });

    // format a new [dependencies]-block with the new deps we need to inject
    let mut all_deps = String::from("[target.'cfg(NOT_A_PLATFORM)'.dependencies]\n");
    new_deps.for_each(|dep_line| {
        all_deps.push_str(&dep_line);
    });
    all_deps.push_str("\n[dependencies]\n");

    // replace "[dependencies]" with
    // [dependencies]
    // dep1 = { path = ... }
    // dep2 = { path = ... }
    // etc
    let new_manifest = cargo_toml.replacen("[dependencies]\n", &all_deps, 1);

    // println!("{}", new_manifest);
    let mut file = File::create(manifest_path)?;
    file.write_all(new_manifest.as_bytes())?;

    println!("info: successfully setup dependencies inside {}", manifest_path);

    Ok(())
}

pub fn remove_rustc_src() {
    for project in CLIPPY_PROJECTS {
        // We don't care about the result here as we want to go through all
        // dependencies either way. Any info and error message will be issued by
        // the removal code itself.
        let _ = remove_rustc_src_from_project(project);
    }
}

fn remove_rustc_src_from_project(project: &ClippyProjectInfo) -> Result<(), ()> {
    let mut cargo_content = read_project_file(project.cargo_file, "Cargo.toml", project.name)?;
    let section_start = if let Some(section_start) = cargo_content.find(RUSTC_PATH_SECTION) {
        section_start
    } else {
        println!(
            "info: dependencies could not be found in `{}` for {}, skipping file",
            project.cargo_file, project.name
        );
        return Ok(());
    };

    let end_point = if let Some(end_point) = cargo_content.find(DEPENDENCIES_SECTION) {
        end_point
    } else {
        eprintln!(
            "error: the end of the rustc dependencies section could not be found in `{}`",
            project.cargo_file
        );
        return Err(());
    };

    cargo_content.replace_range(section_start..end_point, "");

    match File::create(project.cargo_file) {
        Ok(mut file) => {
            file.write_all(cargo_content.as_bytes()).unwrap();
            println!("info: successfully removed dependencies inside {}", project.cargo_file);
            Ok(())
        },
        Err(err) => {
            eprintln!(
                "error: unable to open file `{}` to remove rustc dependencies for {} ({})",
                project.cargo_file, project.name, err
            );
            Err(())
        },
    }
}
