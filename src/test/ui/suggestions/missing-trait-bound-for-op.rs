// run-rustfix

pub fn strip_prefix<'a, T>(s: &'a [T], prefix: &[T]) -> Option<&'a [T]> {
    let n = prefix.len();
    if n <= s.len() {
        let (head, tail) = s.split_at(n);
        if head == prefix { //~ ERROR binary operation `==` cannot be applied to type `&[T]`
            return Some(tail);
        }
    }
    None
}
fn main() {}
