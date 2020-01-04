//~^ WARN the feature `const_generics` is incomplete
#![feature(lazy_normalization_consts)]
//~^ WARN the feature `lazy_normalization_consts` is incomplete

// build-pass

fn f<T: Copy, const N: usize>(x: T) -> [T; N] {
    [x; N]
}

fn main() {
    let x: [u32; 5] = f::<u32, 5>(3);
    assert_eq!(x, [3u32; 5]);
}
