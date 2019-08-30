enum Foo {
    A = 5,
    B = 42,
}
enum Bar {
    C = 42,
    D = 99,
}
#[repr(C)]
union Union {
    foo: &'static Foo,
    bar: &'static Bar,
    u8: &'static u8,
}
static BAR: u8 = 5;
static FOO: (&Foo, &Bar) = unsafe {( //~ undefined behavior
    Union { u8: &BAR }.foo,
    Union { u8: &BAR }.bar,
)};
static FOO2: (&Foo, &Bar) = unsafe {(std::mem::transmute(&BAR), std::mem::transmute(&BAR))};
//~^ undefined behavior

fn main() {}
