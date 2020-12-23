// Run clippy on a fixed set of crates and collect the warnings.
// This helps observing the impact clippy changs have on a set of real-world code.
//
// When a new lint is introduced, we can search the results for new warnings and check for false
// positives.

#![allow(clippy::filter_map)]

use crate::clippy_project_root;

use std::collections::HashMap;
use std::process::Command;
use std::{fs::write, path::PathBuf};

use serde::{Deserialize, Serialize};

// use this to store the crates when interacting with the crates.toml file
#[derive(Debug, Serialize, Deserialize)]
struct CrateList {
    crates: HashMap<String, Vec<String>>,
}

// crate data we stored in the toml, can have multiple versions per crate
// A single TomlCrate is laster mapped to several CrateSources in that case
struct TomlCrate {
    name: String,
    versions: Vec<String>,
}

// represents an archive we download from crates.io
#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
struct CrateSource {
    name: String,
    version: String,
}

// represents the extracted sourcecode of a crate
#[derive(Debug)]
struct Crate {
    version: String,
    name: String,
    // path to the extracted sources that clippy can check
    path: PathBuf,
}

impl CrateSource {
    fn download_and_extract(&self) -> Crate {
        let extract_dir = PathBuf::from("target/crater/crates");
        let krate_download_dir = PathBuf::from("target/crater/downloads");

        // url to download the crate from crates.io
        let url = format!(
            "https://crates.io/api/v1/crates/{}/{}/download",
            self.name, self.version
        );
        println!("Downloading and extracting {} {} from {}", self.name, self.version, url);
        let _ = std::fs::create_dir("target/crater/");
        let _ = std::fs::create_dir(&krate_download_dir);
        let _ = std::fs::create_dir(&extract_dir);

        let krate_file_path = krate_download_dir.join(format!("{}-{}.crate.tar.gz", &self.name, &self.version));
        // don't download/extract if we already have done so
        if !krate_file_path.is_file() {
            // create a file path to download and write the crate data into
            let mut krate_dest = std::fs::File::create(&krate_file_path).unwrap();
            let mut krate_req = ureq::get(&url).call().unwrap().into_reader();
            // copy the crate into the file
            std::io::copy(&mut krate_req, &mut krate_dest).unwrap();

            // unzip the tarball
            let ungz_tar = flate2::read::GzDecoder::new(std::fs::File::open(&krate_file_path).unwrap());
            // extract the tar archive
            let mut archive = tar::Archive::new(ungz_tar);
            archive.unpack(&extract_dir).expect("Failed to extract!");
        }
        // crate is extracted, return a new Krate object which contains the path to the extracted
        // sources that clippy can check
        Crate {
            version: self.version.clone(),
            name: self.name.clone(),
            path: extract_dir.join(format!("{}-{}/", self.name, self.version)),
        }
    }
}

impl Crate {
    fn run_clippy_lints(&self, cargo_clippy_path: &PathBuf) -> Vec<String> {
        println!("Linting {} {}...", &self.name, &self.version);
        let cargo_clippy_path = std::fs::canonicalize(cargo_clippy_path).unwrap();

        let shared_target_dir = clippy_project_root().join("target/crater/shared_target_dir/");

        let all_output = std::process::Command::new(cargo_clippy_path)
            .env("CARGO_TARGET_DIR", shared_target_dir)
            // lint warnings will look like this:
            // src/cargo/ops/cargo_compile.rs:127:35: warning: usage of `FromIterator::from_iter`
            .args(&[
                "--",
                "--message-format=short",
                "--",
                "--cap-lints=warn",
                "-Wclippy::pedantic",
                "-Wclippy::cargo",
            ])
            .current_dir(&self.path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&all_output.stderr);
        let output_lines = stderr.lines();
        let mut output: Vec<String> = output_lines
            .into_iter()
            .filter(|line| line.contains(": warning: "))
            // prefix with the crate name and version
            // cargo-0.49.0/src/cargo/ops/cargo_compile.rs:127:35: warning: usage of `FromIterator::from_iter`
            .map(|line| format!("{}-{}/{}", self.name, self.version, line))
            // remove the "warning: "
            .map(|line| {
                let remove_pat = "warning: ";
                let pos = line
                    .find(&remove_pat)
                    .expect("clippy output did not contain \"warning: \"");
                let mut new = line[0..pos].to_string();
                new.push_str(&line[pos + remove_pat.len()..]);
                new.push('\n');
                new
            })
            .collect();

        // sort messages alphabetically to avoid noise in the logs
        output.sort();
        output
    }
}

fn build_clippy() {
    Command::new("cargo")
        .arg("build")
        .output()
        .expect("Failed to build clippy!");
}

// get a list of CrateSources we want to check from a "crater_crates.toml" file.
fn read_crates() -> Vec<CrateSource> {
    let toml_path = PathBuf::from("clippy_dev/crater_crates.toml");
    let toml_content: String =
        std::fs::read_to_string(&toml_path).unwrap_or_else(|_| panic!("Failed to read {}", toml_path.display()));
    let crate_list: CrateList =
        toml::from_str(&toml_content).unwrap_or_else(|e| panic!("Failed to parse {}: \n{}", toml_path.display(), e));
    // parse the hashmap of the toml file into a list of crates
    let tomlcrates: Vec<TomlCrate> = crate_list
        .crates
        .into_iter()
        .map(|(name, versions)| TomlCrate { name, versions })
        .collect();

    // flatten TomlCrates into CrateSources (one TomlCrates may represent several versions of a crate =>
    // multiple Cratesources)
    let mut crate_sources = Vec::new();
    tomlcrates.into_iter().for_each(|tk| {
        tk.versions.iter().for_each(|ver| {
            crate_sources.push(CrateSource {
                name: tk.name.clone(),
                version: ver.to_string(),
            });
        })
    });
    crate_sources
}

// the main fn
pub fn run() {
    let cargo_clippy_path: PathBuf = PathBuf::from("target/debug/cargo-clippy");

    println!("Compiling clippy...");
    build_clippy();
    println!("Done compiling");

    // assert that clippy is found
    assert!(
        cargo_clippy_path.is_file(),
        "target/debug/cargo-clippy binary not found! {}",
        cargo_clippy_path.display()
    );

    // download and extract the crates, then run clippy on them and collect clippys warnings

    let clippy_lint_results: Vec<Vec<String>> = read_crates()
        .into_iter()
        .map(|krate| krate.download_and_extract())
        .map(|krate| krate.run_clippy_lints(&cargo_clippy_path))
        .collect();

    let mut all_warnings: Vec<String> = clippy_lint_results.into_iter().flatten().collect();
    all_warnings.sort();

    // save the text into mini-crater/logs.txt
    let text = all_warnings.join("");
    write("mini-crater/logs.txt", text).unwrap();
}
