//check-pass
#![feature(capture_disjoint_fields)]
//~^ WARNING: the feature `capture_disjoint_fields` is incomplete
#![warn(unused)]

fn main() {
    let _z = 9;
    let t = (String::from("Hello"), String::from("World"));
    let g = (String::from("Mr"), String::from("Goose"));

    let a = || {
        let (_, g2) = g;
        //~^ WARN unused variable: `g2`
        let c = ||  {
            let (_, t2) = t;
            //~^ WARN unused variable: `t2`
        };

        c();
    };

    a();
}
