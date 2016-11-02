#![feature(plugin, custom_attribute)]
#![plugin(clippy)]
#![deny(cyclomatic_complexity)]
#![deny(unused)]

fn main() {
    kaboom();
}

#[cyclomatic_complexity = "0"]
fn kaboom() {  //~ ERROR: the function has a cyclomatic complexity of 3
    if 42 == 43 {
        panic!();
    } else if "cake" == "lie" {
        println!("what?");
    }
}
