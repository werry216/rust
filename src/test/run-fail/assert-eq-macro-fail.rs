// error-pattern:expected: 15, given: 14

#[deriving(Eq)]
struct Point { x : int }

fn main() {
    assert_eq!(14,15);
}
