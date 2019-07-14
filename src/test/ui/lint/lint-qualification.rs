#![deny(unused_qualifications)]
#[allow(deprecated)]

mod foo {
    pub fn bar() {}
}

fn main() {
    use foo::bar;
    foo::bar(); //~ ERROR: unnecessary qualification
    bar();

    let _ = || -> Result<(), ()> { try!(Ok(())); Ok(()) }; // issue #37345

    macro_rules! m { () => {
        $crate::foo::bar(); // issue #37357
        ::foo::bar(); // issue #38682
    } }
    m!();
}
