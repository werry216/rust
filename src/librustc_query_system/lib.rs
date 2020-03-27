#![feature(bool_to_option)]
#![feature(const_fn)]
#![feature(const_if_match)]
#![feature(const_panic)]
#![feature(core_intrinsics)]
#![feature(hash_raw_entry)]
#![feature(specialization)]
#![feature(stmt_expr_attributes)]
#![feature(vec_remove_item)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate rustc_data_structures;

pub mod dep_graph;
pub mod query;

pub trait HashStableContext {}
