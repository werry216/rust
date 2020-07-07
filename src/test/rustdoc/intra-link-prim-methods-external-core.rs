// aux-build:my-core.rs
// build-aux-docs
// ignore-cross-compile

#![deny(intra_doc_link_resolution_failure)]
#![feature(no_core, lang_items)]
#![no_core]

// @has intra_link_prim_methods_external_core/index.html
// @has - '//*[@id="main"]//a[@href="https://doc.rust-lang.org/nightly/std/primitive.char.html"]' 'char'
// @has - '//*[@id="main"]//a[@href="https://doc.rust-lang.org/nightly/std/primitive.char.html#method.len_utf8"]' 'char::len_utf8'

//! A [`char`] and its [`char::len_utf8`].

extern crate my_core;
