// compile-flags: -Z continue-parse-after-error

pub fn main() {
    let s = "\u{lol}";
     //~^ ERROR invalid character in unicode escape: l
}
