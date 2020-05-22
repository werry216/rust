mod foo {
    pub mod bar {
        pub fn drop() {}
    }
}

use foo::bar::self;
//~^ ERROR `self` imports are only allowed within a { } list

fn main() {
    // Because of error recovery this shouldn't error
    bar::drop();
}
