#![warn(clippy::needless_range_loop, clippy::manual_memcpy)]

const LOOP_OFFSET: usize = 5000;

pub fn manual_copy(src: &[i32], dst: &mut [i32], dst2: &mut [i32]) {
    // plain manual memcpy
    for i in 0..src.len() {
        dst[i] = src[i];
    }

    // dst offset memcpy
    for i in 0..src.len() {
        dst[i + 10] = src[i];
    }

    // src offset memcpy
    for i in 0..src.len() {
        dst[i] = src[i + 10];
    }

    // src offset memcpy
    for i in 11..src.len() {
        dst[i] = src[i - 10];
    }

    // overwrite entire dst
    for i in 0..dst.len() {
        dst[i] = src[i];
    }

    // manual copy with branch - can't easily convert to memcpy!
    for i in 0..src.len() {
        dst[i] = src[i];
        if dst[i] > 5 {
            break;
        }
    }

    // multiple copies - suggest two memcpy statements
    for i in 10..256 {
        dst[i] = src[i - 5];
        dst2[i + 500] = src[i]
    }

    // this is a reversal - the copy lint shouldn't be triggered
    for i in 10..LOOP_OFFSET {
        dst[i + LOOP_OFFSET] = src[LOOP_OFFSET - i];
    }

    let some_var = 5;
    // Offset in variable
    for i in 10..LOOP_OFFSET {
        dst[i + LOOP_OFFSET] = src[i - some_var];
    }

    // Non continuous copy - don't trigger lint
    for i in 0..10 {
        dst[i + i] = src[i];
    }

    let src_vec = vec![1, 2, 3, 4, 5];
    let mut dst_vec = vec![0, 0, 0, 0, 0];

    // make sure vectors are supported
    for i in 0..src_vec.len() {
        dst_vec[i] = src_vec[i];
    }

    // lint should not trigger when either
    // source or destination type is not
    // slice-like, like DummyStruct
    struct DummyStruct(i32);

    impl ::std::ops::Index<usize> for DummyStruct {
        type Output = i32;

        fn index(&self, _: usize) -> &i32 {
            &self.0
        }
    }

    let src = DummyStruct(5);
    let mut dst_vec = vec![0; 10];

    for i in 0..10 {
        dst_vec[i] = src[i];
    }

    // Simplify suggestion (issue #3004)
    let src = [0, 1, 2, 3, 4];
    let mut dst = [0, 0, 0, 0, 0, 0];
    let from = 1;

    for i in from..from + src.len() {
        dst[i] = src[i - from];
    }

    for i in from..from + 3 {
        dst[i] = src[i - from];
    }

    #[allow(clippy::identity_op)]
    for i in 0..5 {
        dst[i - 0] = src[i];
    }

    #[allow(clippy::reversed_empty_ranges)]
    for i in 0..0 {
        dst[i] = src[i];
    }

    // `RangeTo` `for` loop - don't trigger lint
    for i in 0.. {
        dst[i] = src[i];
    }
}

pub fn manual_copy_with_counters(src: &[i32], dst: &mut [i32], dst2: &mut [i32]) {
    let mut count = 0;
    for i in 3..src.len() {
        dst[i] = src[count];
        count += 1;
    }

    let mut count = 0;
    for i in 3..src.len() {
        dst[count] = src[i];
        count += 1;
    }

    let mut count = 3;
    for i in 0..src.len() {
        dst[count] = src[i];
        count += 1;
    }

    let mut count = 3;
    for i in 0..src.len() {
        dst[i] = src[count];
        count += 1;
    }

    let mut count = 0;
    for i in 3..(3 + src.len()) {
        dst[i] = src[count];
        count += 1;
    }

    let mut count = 3;
    for i in 5..src.len() {
        dst[i] = src[count - 2];
        count += 1;
    }

    let mut count = 5;
    for i in 3..10 {
        dst[i] = src[count];
        count += 1;
    }

    let mut count = 3;
    let mut count2 = 30;
    for i in 0..src.len() {
        dst[count] = src[i];
        dst2[count2] = src[i];
        count += 1;
        count2 += 1;
    }

    // make sure parentheses are added properly to bitwise operators, which have lower precedence than
    // arithmetric ones
    let mut count = 0 << 1;
    for i in 0..1 << 1 {
        dst[count] = src[i + 2];
        count += 1;
    }
}

#[warn(clippy::needless_range_loop, clippy::manual_memcpy)]
pub fn manual_clone(src: &[String], dst: &mut [String]) {
    for i in 0..src.len() {
        dst[i] = src[i].clone();
    }
}

fn main() {}
