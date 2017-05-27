#[derive(PartialEq, Debug)]
struct A;

fn zst_ret() -> A {
    A
}

fn use_zst() -> A {
    let a = A;
    a
}

fn main() {
    assert_eq!(zst_ret(), A);
    assert_eq!(use_zst(), A);
    assert_eq!(&A as *const A as *const (), &() as *const _);
    assert_eq!(&A as *const A, &A as *const A);
    let x = 42 as *mut ();
    unsafe { *x = (); }
}
