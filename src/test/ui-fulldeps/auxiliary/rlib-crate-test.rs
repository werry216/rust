// no-prefer-dynamic

#![crate_type = "rlib"]
#![feature(plugin_registrar, rustc_private)]

extern crate rustc;
extern crate rustc_plugin;
extern crate rustc_driver;

use rustc_plugin::Registry;

#[plugin_registrar]
pub fn plugin_registrar(_: &mut Registry) {}
