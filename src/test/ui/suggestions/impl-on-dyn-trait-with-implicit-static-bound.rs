// run-rustfix
#![allow(dead_code)]

mod foo {
    trait OtherTrait<'a> {}
    impl<'a> OtherTrait<'a> for &'a () {}

    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }

    impl MyTrait for dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> impl OtherTrait<'a> + 'a {
        val.use_self() //~ ERROR cannot infer an appropriate lifetime
    }
}

mod bar {
    trait ObjectTrait {}
    trait MyTrait {
        fn use_self(&self) -> &();
    }

    impl MyTrait for dyn ObjectTrait {
        fn use_self(&self) -> &() { panic!() }
    }

    fn use_it<'a>(val: &'a dyn ObjectTrait) -> &'a () {
        val.use_self() //~ ERROR cannot infer an appropriate lifetime
    }
}

fn main() {}
