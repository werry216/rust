// rustfmt-wrap_comments: true
// rustfmt-normalize_doc_attributes: true

//! Example doc attribute comment

// Long `#[doc = "..."]`
struct A {
    /// xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
    b: i32,
}

/// The `nodes` and `edges` method each return instantiations of `Cow<[T]>` to
/// leave implementers the freedom to create entirely new vectors or to pass
/// back slices into internally owned vectors.
struct B {
    b: i32,
}
