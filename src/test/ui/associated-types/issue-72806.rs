trait Bar {
    type Ok;
    type Sibling: Bar2<Ok=char>;
}
trait Bar2 {
    type Ok;
}

struct Foo;
struct Foo2;

impl Bar for Foo {  //~ ERROR type mismatch resolving `<Foo2 as Bar2>::Ok == char`
    type Ok = ();
    type Sibling = Foo2;
}
impl Bar2 for Foo2 {
    type Ok = u32;
}

fn main() {}
