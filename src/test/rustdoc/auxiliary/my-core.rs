#![feature(no_core, lang_items)]
#![no_core]

#[lang = "char"]
impl char {
    pub fn len_utf8(self) -> usize {
        42
    }
}

#[lang = "sized"]
pub trait Sized {}

#[lang = "clone"]
pub trait Clone: Sized {}

#[lang = "copy"]
pub trait Copy: Clone {}
