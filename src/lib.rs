#![feature(
    btree_range,
    collections,
    collections_bound,
    rustc_private,
    pub_restricted,
    cell_extras,
)]

// From rustc.
#[macro_use] extern crate rustc;
extern crate rustc_borrowck;
extern crate rustc_data_structures;
extern crate rustc_mir;
extern crate rustc_const_math;
extern crate syntax;
#[macro_use] extern crate log;
extern crate log_settings;

// From crates.io.
extern crate byteorder;

mod error;
mod interpreter;
mod memory;
mod primval;

pub use error::{
    EvalError,
    EvalResult,
};

pub use interpreter::{
    EvalContext,
    Frame,
    Lvalue,
    LvalueExtra,
    ResourceLimits,
    StackPopCleanup,
    Value,
    eval_main,
    run_mir_passes,
};

pub use memory::{
    Memory,
    Pointer,
    AllocId,
};

pub use primval::{
    PrimVal,
    PrimValKind,
};
