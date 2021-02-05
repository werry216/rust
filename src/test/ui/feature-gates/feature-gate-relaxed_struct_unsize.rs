// Test that we allow unsizing even if there is an unchanged param in the
// field getting unsized.
struct A<T, U: ?Sized + 'static>(T, B<T, U>);
struct B<T, U: ?Sized>(T, U);

fn main() {
    let x: A<[u32; 1], [u32; 1]> = A([0; 1], B([0; 1], [0; 1]));
    let y: &A<[u32; 1], [u32]> = &x; //~ ERROR mismatched types
    assert_eq!(y.1.1.len(), 1);
}
