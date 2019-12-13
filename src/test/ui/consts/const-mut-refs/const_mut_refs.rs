// run-pass

#![feature(const_mut_refs)]
#![feature(const_fn)]

struct Foo {
    x: usize
}

const fn foo() -> Foo {
    Foo { x: 0 }
}

impl Foo {
    const fn bar(&mut self) -> usize {
        self.x = 1;
        self.x
    }

}

const fn baz(foo: &mut Foo) -> usize {
    let x = &mut foo.x;
    *x = 2;
    *x
}

const fn bazz(foo: &mut Foo) -> usize {
    foo.x = 3;
    foo.x
}

fn main() {
    let _: [(); foo().bar()] = [(); 1];
    let _: [(); baz(&mut foo())] = [(); 2];
    let _: [(); bazz(&mut foo())] = [(); 3];
}
