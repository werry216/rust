// run-rustfix

#![warn(clippy::manual_map)]
#![allow(
    clippy::no_effect,
    clippy::map_identity,
    clippy::unit_arg,
    clippy::match_ref_pats,
    dead_code
)]

fn main() {
    match Some(0) {
        Some(_) => Some(2),
        None::<u32> => None,
    };

    match Some(0) {
        Some(x) => Some(x + 1),
        _ => None,
    };

    match Some("") {
        Some(x) => Some(x.is_empty()),
        None => None,
    };

    if let Some(x) = Some(0) {
        Some(!x)
    } else {
        None
    };

    #[rustfmt::skip]
    match Some(0) {
        Some(x) => { Some(std::convert::identity(x)) }
        None => { None }
    };

    match Some(&String::new()) {
        Some(x) => Some(str::len(x)),
        None => None,
    };

    match Some(0) {
        Some(x) if false => Some(x + 1),
        _ => None,
    };

    match &Some([0, 1]) {
        Some(x) => Some(x[0]),
        &None => None,
    };

    match &Some(0) {
        &Some(x) => Some(x * 2),
        None => None,
    };

    match Some(String::new()) {
        Some(ref x) => Some(x.is_empty()),
        _ => None,
    };

    match &&Some(String::new()) {
        Some(x) => Some(x.len()),
        _ => None,
    };

    match &&Some(0) {
        &&Some(x) => Some(x + x),
        &&_ => None,
    };

    #[warn(clippy::option_map_unit_fn)]
    match &mut Some(String::new()) {
        Some(x) => Some(x.push_str("")),
        None => None,
    };

    #[allow(clippy::option_map_unit_fn)]
    {
        match &mut Some(String::new()) {
            Some(x) => Some(x.push_str("")),
            None => None,
        };
    }

    match &mut Some(String::new()) {
        Some(ref x) => Some(x.len()),
        None => None,
    };

    match &mut &Some(String::new()) {
        Some(x) => Some(x.is_empty()),
        &mut _ => None,
    };

    match Some((0, 1, 2)) {
        Some((x, y, z)) => Some(x + y + z),
        None => None,
    };

    match Some([1, 2, 3]) {
        Some([first, ..]) => Some(first),
        None => None,
    };

    match &Some((String::new(), "test")) {
        Some((x, y)) => Some((y, x)),
        None => None,
    };

    match Some((String::new(), 0)) {
        Some((ref x, y)) => Some((y, x)),
        None => None,
    };

    match Some(Some(0)) {
        Some(Some(_)) | Some(None) => Some(0),
        None => None,
    };

    match Some(Some((0, 1))) {
        Some(Some((x, 1))) => Some(x),
        _ => None,
    };

    // #6795
    fn f1() -> Result<(), ()> {
        let _ = match Some(Ok(())) {
            Some(x) => Some(x?),
            None => None,
        };
        Ok(())
    }

    for &x in Some(Some(true)).iter() {
        let _ = match x {
            Some(x) => Some(if x { continue } else { x }),
            None => None,
        };
    }

    // #6797
    let x1 = (Some(String::new()), 0);
    let x2 = x1.0;
    match x2 {
        Some(x) => Some((x, x1.1)),
        None => None,
    };

    struct S1 {
        x: Option<String>,
        y: u32,
    }
    impl S1 {
        fn f(self) -> Option<(String, u32)> {
            match self.x {
                Some(x) => Some((x, self.y)),
                None => None,
            }
        }
    }
}
