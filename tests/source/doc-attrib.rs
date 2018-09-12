// rustfmt-wrap_comments: true
// rustfmt-normalize_doc_attributes: true

#![doc = "Example doc attribute comment"]

// Long `#[doc = "..."]`
struct A { #[doc = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"] b: i32 }


#[doc = "The `nodes` and `edges` method each return instantiations of `Cow<[T]>` to leave implementers the freedom to create entirely new vectors or to pass back slices into internally owned vectors."]
struct B { b: i32 }


#[doc = "Level 1 comment"]
mod tests {
    #[doc = "Level 2 comment"]
    impl A {
        #[doc = "Level 3 comment"]
        fn f() {
            #[doc = "Level 4 comment"]
            fn g() {                
            }
        }
    }
}
