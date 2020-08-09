// Assert that cannot use const generics as ptrs and cannot deref them.
// revisions: full min

#![cfg_attr(full, feature(const_generics))]
#![cfg_attr(full, allow(incomplete_features))]
#![cfg_attr(min, feature(min_const_generics))]

const A: u32 = 3;

struct Const<const P: *const u32>; //~ ERROR: using raw pointers as const generic parameters

impl<const P: *const u32> Const<P> { //~ ERROR: using raw pointers as const generic parameters
    fn get() -> u32 {
        unsafe {
            *P
        }
    }
}

fn main() {
    assert_eq!(Const::<{&A as *const _}>::get(), 3)
}
