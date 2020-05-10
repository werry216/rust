// Regression test for ICE #73061

// aux-build:issue-73061.rs

extern crate issue_73061;

pub struct Z;

impl issue_73061::Foo for Z {
    type X = <issue_73061::F as issue_73061::Foo>::X;
    fn x(&self) -> Self::X {
        issue_73061::F.x()
    }
}
