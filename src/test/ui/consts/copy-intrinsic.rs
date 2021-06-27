#![stable(feature = "dummy", since = "1.0.0")]

// ignore-tidy-linelength
#![feature(intrinsics, staged_api)]
#![feature(const_mut_refs, const_intrinsic_copy, const_ptr_offset)]
use std::mem;

extern "rust-intrinsic" {
    #[rustc_const_unstable(feature = "const_intrinsic_copy", issue = "80697")]
    fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize);

    #[rustc_const_unstable(feature = "const_intrinsic_copy", issue = "80697")]
    fn copy<T>(src: *const T, dst: *mut T, count: usize);
}

const COPY_ZERO: () = unsafe {
    // Since we are not copying anything, this should be allowed.
    let src = ();
    let mut dst = ();
    copy_nonoverlapping(&src as *const _ as *const i32, &mut dst as *mut _ as *mut i32, 0);
};

const COPY_OOB_1: () = unsafe {
    let mut x = 0i32;
    let dangle = (&mut x as *mut i32).wrapping_add(10);
    // Even if the first ptr is an int ptr and this is a ZST copy, we should detect dangling 2nd ptrs.
    copy_nonoverlapping(0x100 as *const i32, dangle, 0); //~ evaluation of constant value failed [E0080]
};
const COPY_OOB_2: () = unsafe {
    let x = 0i32;
    let dangle = (&x as *const i32).wrapping_add(10);
    // Even if the second ptr is an int ptr and this is a ZST copy, we should detect dangling 1st ptrs.
    copy_nonoverlapping(dangle, 0x100 as *mut i32, 0); //~ evaluation of constant value failed [E0080]
    //~| memory access failed: pointer must be in-bounds
};

const COPY_SIZE_OVERFLOW: () = unsafe {
    let x = 0;
    let mut y = 0;
    copy(&x, &mut y, 1usize << (mem::size_of::<usize>() * 8 - 1)); //~ evaluation of constant value failed [E0080]
    //~| overflow computing total size of `copy`
};
const COPY_NONOVERLAPPING_SIZE_OVERFLOW: () = unsafe {
    let x = 0;
    let mut y = 0;
    copy_nonoverlapping(&x, &mut y, 1usize << (mem::size_of::<usize>() * 8 - 1)); //~ evaluation of constant value failed [E0080]
    //~| overflow computing total size of `copy_nonoverlapping`
};

fn main() {
}
