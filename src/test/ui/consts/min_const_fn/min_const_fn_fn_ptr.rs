// gate-test-const_fn_fn_ptr_basics

struct HasPtr {
    field: fn(),
}

struct Hide(HasPtr);

fn field() {}

const fn no_inner_dyn_trait(_x: Hide) {}
const fn no_inner_dyn_trait2(x: Hide) {
    x.0.field;
//~^ ERROR function pointer
}
const fn no_inner_dyn_trait_ret() -> Hide { Hide(HasPtr { field }) }
//~^ ERROR function pointer

fn main() {}
