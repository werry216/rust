// revisions: min_tait full_tait
#![feature(min_type_alias_impl_trait, rustc_attrs)]
#![cfg_attr(full_tait, feature(type_alias_impl_trait))]
//[full_tait]~^ WARN incomplete

type Debuggable = impl core::fmt::Debug;

static mut TEST: Option<Debuggable> = None;

#[rustc_error]
fn main() {
    //~^ ERROR
    unsafe { TEST = Some(foo()) }
}

fn foo() -> Debuggable {
    0u32
}
