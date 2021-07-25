#![crate_name = "foo"]

pub trait Foo<T> {}

pub struct Bar;

pub struct Baz;

// @has foo/trait.Foo.html '//h3[@class="code-header in-band"]' 'impl Foo<i32> for Bar'
impl Foo<i32> for Bar {}

// @has foo/trait.Foo.html '//h3[@class="code-header in-band"]' 'impl<T> Foo<T> for Baz'
impl<T> Foo<T> for Baz {}
