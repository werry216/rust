#![warn(clippy::same_item_push)]

const VALUE: u8 = 7;

fn mutate_increment(x: &mut u8) -> u8 {
    *x += 1;
    *x
}

fn increment(x: u8) -> u8 {
    x + 1
}

fn fun() -> usize {
    42
}

fn main() {
    // Test for basic case
    let mut spaces = Vec::with_capacity(10);
    for _ in 0..10 {
        spaces.push(vec![b' ']);
    }

    let mut vec2: Vec<u8> = Vec::new();
    let item = 2;
    for _ in 5..=20 {
        vec2.push(item);
    }

    let mut vec3: Vec<u8> = Vec::new();
    for _ in 0..15 {
        let item = 2;
        vec3.push(item);
    }

    let mut vec4: Vec<u8> = Vec::new();
    for _ in 0..15 {
        vec4.push(13);
    }

    // Suggestion should not be given as pushed variable can mutate
    let mut vec5: Vec<u8> = Vec::new();
    let mut item: u8 = 2;
    for _ in 0..30 {
        vec5.push(mutate_increment(&mut item));
    }

    let mut vec6: Vec<u8> = Vec::new();
    let mut item: u8 = 2;
    let mut item2 = &mut mutate_increment(&mut item);
    for _ in 0..30 {
        vec6.push(mutate_increment(item2));
    }

    let mut vec7: Vec<usize> = Vec::new();
    for (a, b) in [0, 1, 4, 9, 16].iter().enumerate() {
        vec7.push(a);
    }

    let mut vec8: Vec<u8> = Vec::new();
    for i in 0..30 {
        vec8.push(increment(i));
    }

    let mut vec9: Vec<u8> = Vec::new();
    for i in 0..30 {
        vec9.push(i + i * i);
    }

    // Suggestion should not be given as there are multiple pushes that are not the same
    let mut vec10: Vec<u8> = Vec::new();
    let item: u8 = 2;
    for _ in 0..30 {
        vec10.push(item);
        vec10.push(item * 2);
    }

    // Suggestion should not be given as Vec is not involved
    for _ in 0..5 {
        println!("Same Item Push");
    }

    struct A {
        kind: u32,
    }
    let mut vec_a: Vec<A> = Vec::new();
    for i in 0..30 {
        vec_a.push(A { kind: i });
    }
    let mut vec12: Vec<u8> = Vec::new();
    for a in vec_a {
        vec12.push(2u8.pow(a.kind));
    }

    // Fix #5902
    let mut vec13: Vec<u8> = Vec::new();
    let mut item = 0;
    for _ in 0..10 {
        vec13.push(item);
        item += 10;
    }

    // Fix #5979
    let mut vec14: Vec<std::fs::File> = Vec::new();
    for _ in 0..10 {
        vec14.push(std::fs::File::open("foobar").unwrap());
    }
    // Fix #5979
    #[derive(Clone)]
    struct S {}

    trait T {}
    impl T for S {}

    let mut vec15: Vec<Box<dyn T>> = Vec::new();
    for _ in 0..10 {
        vec15.push(Box::new(S {}));
    }

    let mut vec16 = Vec::new();
    for _ in 0..20 {
        vec16.push(VALUE);
    }

    let mut vec17 = Vec::new();
    let item = VALUE;
    for _ in 0..20 {
        vec17.push(item);
    }

    let mut vec18 = Vec::new();
    let item = 42;
    let item = fun();
    for _ in 0..20 {
        vec18.push(item);
    }

    let mut vec19 = Vec::new();
    let key = 1;
    for _ in 0..20 {
        let item = match key {
            1 => 10,
            _ => 0,
        };
        vec19.push(item);
    }
}
