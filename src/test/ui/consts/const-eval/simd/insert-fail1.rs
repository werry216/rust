// failure-status: 101
// rustc-env:RUST_BACKTRACE=0
#![feature(const_fn)]
#![feature(repr_simd)]
#![feature(platform_intrinsics)]
#![allow(non_camel_case_types)]

#[repr(simd)] struct i8x1(i8);

extern "platform-intrinsic" {
    fn simd_insert<T, U>(x: T, idx: u32, val: U) -> T;
}

const X: i8x1 = i8x1(42);

const fn insert_wrong_idx() -> i8x1 {
    unsafe { simd_insert(X, 1_u32, 42_i8) }
}

const E: i8x1 = insert_wrong_idx();

fn main() {}
