#![crate_name = "bar"]
#![deny(broken_intra_doc_links)]

pub trait Foo {
    /// [`Bar`] [`Baz`]
    fn foo();
}

pub trait Bar {
}

pub trait Baz {
}
