// force-host
// no-prefer-dynamic

#![crate_type = "proc-macro"]
#![feature(proc_macro_quote)]

extern crate proc_macro;
use proc_macro::{TokenStream, quote};

#[proc_macro_attribute]
pub fn first_attr(_: TokenStream, input: TokenStream) -> TokenStream {
    let recollected: TokenStream = input.into_iter().collect();
    quote! {
        #[second_attr]
        $recollected
    }
}

#[proc_macro_attribute]
pub fn second_attr(_: TokenStream, input: TokenStream) -> TokenStream {
    let _recollected: TokenStream = input.into_iter().collect();
    TokenStream::new()
}
