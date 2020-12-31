// #26207: Show all methods reachable via Deref impls, recursing through multiple dereferencing
// levels if needed.

// @has 'foo/struct.Foo.html'
// @has '-' '//*[@id="deref-methods"]' 'Methods from Deref<Target = Bar>'
// @has '-' '//*[@class="impl-items"]//*[@id="method.bar"]' 'pub fn bar(&self)'
// @has '-' '//*[@id="deref-methods"]' 'Methods from Deref<Target = Baz>'
// @has '-' '//*[@class="impl-items"]//*[@id="method.baz"]' 'pub fn baz(&self)'
// @has '-' '//*[@class="sidebar-title"]' 'Methods from Deref<Target=Bar>'
// @has '-' '//*[@class="sidebar-links"]/a[@href="#method.bar"]' 'bar'
// @has '-' '//*[@class="sidebar-title"]' 'Methods from Deref<Target=Baz>'
// @has '-' '//*[@class="sidebar-links"]/a[@href="#method.baz"]' 'baz'

#![crate_name = "foo"]

use std::ops::Deref;

pub struct Foo(Bar);
pub struct Bar(Baz);
pub struct Baz;

impl Deref for Foo {
    type Target = Bar;
    fn deref(&self) -> &Bar { &self.0 }
}

impl Deref for Bar {
    type Target = Baz;
    fn deref(&self) -> &Baz { &self.0 }
}

impl Bar {
    /// This appears under `Foo` methods
    pub fn bar(&self) {}
}

impl Baz {
    /// This should also appear in `Foo` methods when recursing
    pub fn baz(&self) {}
}
