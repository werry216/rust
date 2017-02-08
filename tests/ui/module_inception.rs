#![feature(plugin)]
#![plugin(clippy)]
#![deny(module_inception)]

mod foo {
    mod bar {
        mod bar {
            mod foo {}
        }
        mod foo {}
    }
    mod foo {
        mod bar {}
    }
}

// No warning. See <https://github.com/Manishearth/rust-clippy/issues/1220>.
mod bar {
    #[allow(module_inception)]
    mod bar {
    }
}

fn main() {}
