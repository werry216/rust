#![feature(repr_simd)]
#![allow(non_camel_case_types)]

// ignore-tidy-linelength

#[repr(simd)]
struct empty; //~ ERROR SIMD vector cannot be empty

#[repr(simd)]
struct empty2([f32; 0]); //~ ERROR SIMD vector cannot be empty

#[repr(simd)]
struct pow2([f32; 7]); //~ ERROR SIMD vector length must be a power of two

#[repr(simd)]
struct i64f64(i64, f64); //~ ERROR SIMD vector should be homogeneous

struct Foo;

#[repr(simd)]
struct FooV(Foo, Foo); //~ ERROR SIMD vector element type should be a primitive scalar (integer/float/pointer) type

#[repr(simd)]
struct FooV2([Foo; 2]); //~ ERROR SIMD vector element type should be a primitive scalar (integer/float/pointer) type

#[repr(simd)]
struct TooBig([f32; 65536]); //~ ERROR SIMD vector cannot have more than 32768 elements

#[repr(simd)]
struct JustRight([u128; 32768]);

fn main() {}
