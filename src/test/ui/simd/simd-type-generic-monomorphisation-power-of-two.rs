// run-pass

#![feature(repr_simd, platform_intrinsics)]

#[repr(simd)]
struct Simd<const N: usize>([f32; N]);

fn main() {
    let _ = Simd::<3>([0.; 3]);
}
