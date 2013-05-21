// Tests that we are able to distinguish when loans borrow different
// anonymous fields of a tuple vs the same anonymous field.

fn distinct_variant() {
    let mut y = (1, 2);

    let a = match y {
        (ref mut a, _) => a
    };

    let b = match y {
        (_, ref mut b) => b
    };

    *a += 1;
    *b += 1;
}

fn same_variant() {
    let mut y = (1, 2);

    let a = match y {
        (ref mut a, _) => a
    };

    let b = match y {
        (ref mut b, _) => b //~ ERROR cannot borrow
    };

    *a += 1;
    *b += 1;
}

fn main() {
}