#![feature(doc_alias)]

#![crate_type = "lib"]

#![doc(alias = "shouldn't work!")] //~ ERROR
