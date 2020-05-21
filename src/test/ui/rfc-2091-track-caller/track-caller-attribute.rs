// run-pass

use std::panic::Location;

#[track_caller]
fn tracked() -> &'static Location<'static> {
    Location::caller()
}

fn nested_intrinsic() -> &'static Location<'static> {
    Location::caller()
}

fn nested_tracked() -> &'static Location<'static> {
    tracked()
}

fn main() {
    let location = Location::caller();
    assert_eq!(location.file(), file!());
    assert_eq!(location.line(), 19);
    assert_eq!(location.column(), 20);

    let tracked = tracked();
    assert_eq!(tracked.file(), file!());
    assert_eq!(tracked.line(), 24);
    assert_eq!(tracked.column(), 19);

    let nested = nested_intrinsic();
    assert_eq!(nested.file(), file!());
    assert_eq!(nested.line(), 11);
    assert_eq!(nested.column(), 5);

    let contained = nested_tracked();
    assert_eq!(contained.file(), file!());
    assert_eq!(contained.line(), 15);
    assert_eq!(contained.column(), 5);
}
