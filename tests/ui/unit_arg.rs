#![warn(unit_arg)]
#![allow(no_effect)]

use std::fmt::Debug;

fn foo<T: Debug>(t: T) {
    println!("{:?}", t);
}

fn foo3<T1: Debug, T2: Debug, T3: Debug>(t1: T1, t2: T2, t3: T3) {
    println!("{:?}, {:?}, {:?}", t1, t2, t3);
}

struct Bar;

impl Bar {
    fn bar<T: Debug>(&self, t: T) {
        println!("{:?}", t);
    }
}

fn bad() {
    foo({});
    foo({ 1; });
    foo(foo(1));
    foo({
        foo(1);
        foo(2);
    });
    foo3({}, 2, 2);
    let b = Bar;
    b.bar({ 1; });
}

fn ok() {
    foo(());
    foo(1);
    foo({ 1 });
    foo3("a", 3, vec![3]);
    let b = Bar;
    b.bar({ 1 });
    b.bar(());
    question_mark();
}

fn question_mark() -> Result<(), ()> {
    Ok(Ok(())?)?;
    Ok(Ok(()))??;
    Ok(())
}

fn main() {
    bad();
    ok();
}
