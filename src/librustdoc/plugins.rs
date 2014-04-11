// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clean;

use dl = std::unstable::dynamic_lib;
use serialize::json;
use std::strbuf::StrBuf;

pub type PluginJson = Option<(~str, json::Json)>;
pub type PluginResult = (clean::Crate, PluginJson);
pub type PluginCallback = fn (clean::Crate) -> PluginResult;

/// Manages loading and running of plugins
pub struct PluginManager {
    dylibs: Vec<dl::DynamicLibrary> ,
    callbacks: Vec<PluginCallback> ,
    /// The directory plugins will be loaded from
    pub prefix: Path,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(prefix: Path) -> PluginManager {
        PluginManager {
            dylibs: Vec::new(),
            callbacks: Vec::new(),
            prefix: prefix,
        }
    }

    /// Load a plugin with the given name.
    ///
    /// Turns `name` into the proper dynamic library filename for the given
    /// platform. On windows, it turns into name.dll, on OS X, name.dylib, and
    /// elsewhere, libname.so.
    pub fn load_plugin(&mut self, name: ~str) {
        let x = self.prefix.join(libname(name));
        let lib_result = dl::DynamicLibrary::open(Some(&x));
        let lib = lib_result.unwrap();
        let plugin = unsafe { lib.symbol("rustdoc_plugin_entrypoint") }.unwrap();
        self.dylibs.push(lib);
        self.callbacks.push(plugin);
    }

    /// Load a normal Rust function as a plugin.
    ///
    /// This is to run passes over the cleaned crate. Plugins run this way
    /// correspond to the A-aux tag on Github.
    pub fn add_plugin(&mut self, plugin: PluginCallback) {
        self.callbacks.push(plugin);
    }
    /// Run all the loaded plugins over the crate, returning their results
    pub fn run_plugins(&self, krate: clean::Crate) -> (clean::Crate, Vec<PluginJson> ) {
        let mut out_json = Vec::new();
        let mut krate = krate;
        for &callback in self.callbacks.iter() {
            let (c, res) = callback(krate);
            krate = c;
            out_json.push(res);
        }
        (krate, out_json)
    }
}

#[cfg(target_os="win32")]
fn libname(n: ~str) -> ~str {
    let mut n = StrBuf::from_owned_str(n);
    n.push_str(".dll");
    n.into_owned()
}

#[cfg(target_os="macos")]
fn libname(n: ~str) -> ~str {
    let mut n = StrBuf::from_owned_str(n);
    n.push_str(".dylib");
    n.into_owned()
}

#[cfg(not(target_os="win32"), not(target_os="macos"))]
fn libname(n: ~str) -> ~str {
    let mut i = StrBuf::from_str("lib");
    i.push_str(n);
    i.push_str(".so");
    i.into_owned()
}
