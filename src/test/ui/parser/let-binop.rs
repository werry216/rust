fn main() {
    let a: i8 *= 1; //~ ERROR can't reassign to an uninitialized variable
    let _ = a;
    let b += 1; //~ ERROR can't reassign to an uninitialized variable
    let _ = b;
    let c *= 1; //~ ERROR can't reassign to an uninitialized variable
    let _ = c;
}
