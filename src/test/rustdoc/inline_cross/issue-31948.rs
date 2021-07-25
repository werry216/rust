// aux-build:rustdoc-nonreachable-impls.rs
// build-aux-docs
// ignore-cross-compile

extern crate rustdoc_nonreachable_impls;

// @has issue_31948/struct.Foo.html
// @has - '//*[@class="impl has-srclink"]//h3[@class="code-header in-band"]' 'Bark for'
// @has - '//*[@class="impl has-srclink"]//h3[@class="code-header in-band"]' 'Woof for'
// @!has - '//*[@class="impl has-srclink"]//h3[@class="code-header in-band"]' 'Bar for'
// @!has - '//*[@class="impl"]//h3[@class="code-header in-band"]' 'Qux for'
pub use rustdoc_nonreachable_impls::Foo;

// @has issue_31948/trait.Bark.html
// @has - '//h3[@class="code-header in-band"]' 'for Foo'
// @!has - '//h3[@class="code-header in-band"]' 'for Wibble'
// @!has - '//h3[@class="code-header in-band"]' 'for Wobble'
pub use rustdoc_nonreachable_impls::Bark;

// @has issue_31948/trait.Woof.html
// @has - '//h3[@class="code-header in-band"]' 'for Foo'
// @!has - '//h3[@class="code-header in-band"]' 'for Wibble'
// @!has - '//h3[@class="code-header in-band"]' 'for Wobble'
pub use rustdoc_nonreachable_impls::Woof;

// @!has issue_31948/trait.Bar.html
// @!has issue_31948/trait.Qux.html
// @!has issue_31948/struct.Wibble.html
// @!has issue_31948/struct.Wobble.html
