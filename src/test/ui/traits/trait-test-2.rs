#![feature(box_syntax)]

#[allow(non_camel_case_types)]
trait bar { fn dup(&self) -> Self; fn blah<X>(&self); }
impl bar for i32 { fn dup(&self) -> i32 { *self } fn blah<X>(&self) {} }
impl bar for u32 { fn dup(&self) -> u32 { *self } fn blah<X>(&self) {} }

fn main() {
    10.dup::<i32>();
    //~^ ERROR this associated function takes 0 type arguments but 1 type argument was supplied
    10.blah::<i32, i32>();
    //~^ ERROR this associated function takes 1 type argument but 2 type arguments were supplied
    (box 10 as Box<dyn bar>).dup();
    //~^ ERROR E0038
    //~| ERROR E0038
}
