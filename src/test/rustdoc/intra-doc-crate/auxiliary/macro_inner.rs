#![crate_name = "macro_inner"]
#![deny(intra_doc_resolution_failures)]

pub struct Foo;

/// See also [`Foo`]
#[macro_export]
macro_rules! my_macro {
    () => {}
}
