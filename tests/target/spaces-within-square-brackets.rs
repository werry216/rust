// rustfmt-spaces_within_square_brackets: true

fn main() {

    let arr: [ i32; 5 ] = [ 1, 2, 3, 4, 5 ];
    let arr: [ i32; 500 ] = [ 0; 500 ];

    let v = vec![ 1, 2, 3 ];
    assert_eq!(arr, [ 1, 2, 3 ]);

    let i = arr[ 0 ];

    let slice = &arr[ 1..2 ];

    let line100_________________________________________________________________________ = [ 1, 2 ];
    let line101__________________________________________________________________________ =
        [ 1, 2 ];
    let line102___________________________________________________________________________ =
        [ 1, 2 ];
    let line103____________________________________________________________________________ =
        [ 1, 2 ];
    let line104_____________________________________________________________________________ =
        [ 1, 2 ];

    let line100_____________________________________________________________________ = vec![ 1, 2 ];
    let line101______________________________________________________________________ =
        vec![ 1, 2 ];
    let line102_______________________________________________________________________ =
        vec![ 1, 2 ];
    let line103________________________________________________________________________ =
        vec![ 1, 2 ];
    let line104_________________________________________________________________________ =
        vec![ 1, 2 ];
}

fn f(slice: &[ i32 ]) {}
