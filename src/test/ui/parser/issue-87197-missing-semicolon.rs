// run-rustfix
// Parser should know when a semicolon is missing.
// https://github.com/rust-lang/rust/issues/87197

fn main() {
    let x = 100 //~ ERROR: expected `;`
    println!("{}", x) //~ ERROR: expected `;`
    let y = 200 //~ ERROR: expected `;`
    println!("{}", y);
}
