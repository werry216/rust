//check-pass
#![feature(capture_disjoint_fields)]
//~^ WARNING: the feature `capture_disjoint_fields` is incomplete
#![warn(unused)]

struct Point {
    x: u32,
    y: u32,
}

fn test1() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));

    let c = ||  {
        let (t1, t2) = t;
        println!("{} {}", t1, t2);
    };

    c();
}

fn test2() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));

    let c = ||  {
        let (t1, _) = t;
        println!("{}", t1);
    };

    c();
}

fn test3() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));

    let c = ||  {
        let (_, t2) = t;
        println!("{}", t2);
    };

    c();
}

fn test4() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));
    //~^ WARN unused variable: `t`

    let c = ||  {
        let (_, _) = t;
    };

    c();
}

fn test5() {
    let _z = 9;
    let t = (String::new(), String::new());
    let _c = ||  {
        let _a = match t {
            (t1, _) => t1,
        };
    };
}

fn test6() {
    let _z = 9;
    let t = (String::new(), String::new());
    let _c = ||  {
        let _a = match t {
            (_, t2) => t2,
        };
    };
}

fn test7() {
    let x = 0;
    //~^ WARN unused variable: `x`
    let tup = (1, 2);
    //~^ WARN unused variable: `tup`
    let p = Point { x: 10, y: 20 };

    let c = || {
        let _ = x;
        let Point { x, y } = p; // 1
        //~^ WARN unused variable: `x`
        println!("{}", y);
        let (_, _) = tup; // 2
    };

    c();
}

fn test8() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));

    let c = ||  {
        let (_, t) = t;
        println!("{}", t);
    };

    c();
}

fn main() {
    test1();
    test2();
    test3();
    test4();
    test5();
    test6();
    test7();
    test8();
}
