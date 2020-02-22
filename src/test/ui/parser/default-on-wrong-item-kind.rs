// Test parsing for `default` where it doesn't belong.
// Specifically, we are interested in kinds of items or items in certain contexts.
// Also test item kinds in `extern` blocks and associated contexts which are not allowed there.

fn main() {}

#[cfg(FALSE)]
mod free_items {
    default extern crate foo; //~ ERROR item cannot be `default`
    default use foo; //~ ERROR item cannot be `default`
    default static foo: u8; //~ ERROR item cannot be `default`
    default const foo: u8; //~ ERROR item cannot be `default`
    default fn foo(); //~ ERROR item cannot be `default`
    default mod foo {} //~ ERROR item cannot be `default`
    default extern "C" {} //~ ERROR item cannot be `default`
    default type foo = u8; //~ ERROR item cannot be `default`
    default enum foo {} //~ ERROR item cannot be `default`
    default struct foo {} //~ ERROR item cannot be `default`
    default union foo {} //~ ERROR item cannot be `default`
    default trait foo {} //~ ERROR item cannot be `default`
    default trait foo = Ord; //~ ERROR item cannot be `default`
    default impl foo {}
    default!();
    default::foo::bar!();
    default macro foo {} //~ ERROR item cannot be `default`
    default macro_rules! foo {} //~ ERROR item cannot be `default`
}

#[cfg(FALSE)]
extern "C" {
    default extern crate foo; //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default use foo; //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default static foo: u8; //~ ERROR item cannot be `default`
    default const foo: u8; //~ ERROR item cannot be `default`
    //~^ ERROR extern items cannot be `const`
    default fn foo(); //~ ERROR item cannot be `default`
    default mod foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default extern "C" {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default type foo = u8; //~ ERROR item cannot be `default`
    default enum foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default struct foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default union foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default trait foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default trait foo = Ord; //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default impl foo {}
    //~^ ERROR item kind not supported in `extern` block
    default!();
    default::foo::bar!();
    default macro foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
    default macro_rules! foo {} //~ ERROR item cannot be `default`
    //~^ ERROR item kind not supported in `extern` block
}
