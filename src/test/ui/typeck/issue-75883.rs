// Regression test for #75883.

pub struct UI {}

impl UI {
    pub fn run() -> Result<_> {
        //~^ ERROR: this enum takes 2 type arguments but only 1 type argument was supplied
        //~| ERROR: the type placeholder `_` is not allowed within types on item signatures
        let mut ui = UI {};
        ui.interact();

        unimplemented!();
    }

    pub fn interact(&mut self) -> Result<_> {
        //~^ ERROR: this enum takes 2 type arguments but only 1 type argument was supplied
        //~| ERROR: the type placeholder `_` is not allowed within types on item signatures
        unimplemented!();
    }
}

fn main() {}
