use std::libc;
use std::sys;
use std::vec;

extern {
    pub unsafe fn vec_reserve_shared_actual(t: *sys::TypeDesc,
                                            v: **vec::raw::VecRepr,
                                            n: libc::size_t);
}

pub fn main() {
}
