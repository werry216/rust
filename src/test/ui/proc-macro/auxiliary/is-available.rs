// force-host
// no-prefer-dynamic

#![crate_type = "proc-macro"]
#![feature(proc_macro_is_available)]

extern crate proc_macro;

use proc_macro::{Literal, TokenStream, TokenTree};

#[proc_macro]
pub fn from_inside_proc_macro(_input: TokenStream) -> TokenStream {
    proc_macro::is_available().to_string().parse().unwrap()
}
