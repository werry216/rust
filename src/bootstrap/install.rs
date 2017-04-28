// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the install aspects of the compiler.
//!
//! This module is responsible for installing the standard library,
//! compiler, and documentation.

use std::env;
use std::fs;
use std::path::{Path, PathBuf, Component};
use std::process::Command;

use Build;
use dist::{sanitize_sh, tmpdir};

/// Installs everything.
pub fn install(build: &Build, stage: u32, host: &str) {
    let prefix_default = PathBuf::from("/usr/local");
    let docdir_default = PathBuf::from("share/doc/rust");
    let bindir_default = PathBuf::from("bin");
    let libdir_default = PathBuf::from("lib");
    let mandir_default = PathBuf::from("share/man");
    let prefix = build.config.prefix.as_ref().unwrap_or(&prefix_default);
    let docdir = build.config.docdir.as_ref().unwrap_or(&docdir_default);
    let bindir = build.config.bindir.as_ref().unwrap_or(&bindir_default);
    let libdir = build.config.libdir.as_ref().unwrap_or(&libdir_default);
    let mandir = build.config.mandir.as_ref().unwrap_or(&mandir_default);

    let docdir = prefix.join(docdir);
    let bindir = prefix.join(bindir);
    let libdir = prefix.join(libdir);
    let mandir = prefix.join(mandir);

    let destdir = env::var_os("DESTDIR").map(PathBuf::from);

    let prefix = add_destdir(&prefix, &destdir);
    let docdir = add_destdir(&docdir, &destdir);
    let bindir = add_destdir(&bindir, &destdir);
    let libdir = add_destdir(&libdir, &destdir);
    let mandir = add_destdir(&mandir, &destdir);

    let empty_dir = build.out.join("tmp/empty_dir");
    t!(fs::create_dir_all(&empty_dir));
    if build.config.docs {
        install_sh(&build, "docs", "rust-docs", &build.rust_package_vers(),
                   stage, host, &prefix, &docdir, &bindir, &libdir, &mandir, &empty_dir);
    }

    for target in build.config.target.iter() {
        install_sh(&build, "std", "rust-std", &build.rust_package_vers(),
                   stage, target, &prefix, &docdir, &bindir, &libdir, &mandir, &empty_dir);
    }

    if build.config.extended {
        install_sh(&build, "cargo", "cargo", &build.cargo_package_vers(),
                   stage, host, &prefix, &docdir, &bindir, &libdir, &mandir, &empty_dir);
        install_sh(&build, "rls", "rls", &build.rls_package_vers(),
                   stage, host, &prefix, &docdir, &bindir, &libdir, &mandir, &empty_dir);
    }

    install_sh(&build, "rustc", "rustc", &build.rust_package_vers(),
               stage, host, &prefix, &docdir, &bindir, &libdir, &mandir, &empty_dir);

    t!(fs::remove_dir_all(&empty_dir));
}

fn install_sh(build: &Build, package: &str, name: &str, version: &str, stage: u32, host: &str,
              prefix: &Path, docdir: &Path, bindir: &Path, libdir: &Path, mandir: &Path, empty_dir: &Path) {
    println!("Install {} stage{} ({})", package, stage, host);
    let package_name = format!("{}-{}-{}", name, version, host);

    let mut cmd = Command::new("sh");
    cmd.current_dir(empty_dir)
       .arg(sanitize_sh(&tmpdir(build).join(&package_name).join("install.sh")))
       .arg(format!("--prefix={}", sanitize_sh(prefix)))
       .arg(format!("--docdir={}", sanitize_sh(docdir)))
       .arg(format!("--bindir={}", sanitize_sh(bindir)))
       .arg(format!("--libdir={}", sanitize_sh(libdir)))
       .arg(format!("--mandir={}", sanitize_sh(mandir)))
       .arg("--disable-ldconfig");
    build.run(&mut cmd);
}

fn add_destdir(path: &Path, destdir: &Option<PathBuf>) -> PathBuf {
    let mut ret = match *destdir {
        Some(ref dest) => dest.clone(),
        None => return path.to_path_buf(),
    };
    for part in path.components() {
        match part {
            Component::Normal(s) => ret.push(s),
            _ => {}
        }
    }
    return ret
}
