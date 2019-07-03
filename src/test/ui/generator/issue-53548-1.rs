// A variant of #53548 that does not actually require generators,
// but which encountered the same ICE/error. See `issue-53548.rs`
// for details.
//
// build-pass (FIXME(62277): could be check-pass?)

use std::cell::RefCell;
use std::rc::Rc;

trait Trait: 'static {}

struct Store<C> {
    inner: Rc<RefCell<Option<C>>>,
}

fn main() {
    let store = Store::<Box<for<'a> fn(&(dyn Trait + 'a))>> {
        inner: Default::default(),
    };
}
