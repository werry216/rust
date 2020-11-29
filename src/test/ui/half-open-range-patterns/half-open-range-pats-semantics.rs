// run-pass

// Test half-open range patterns against their expression equivalents
// via `.contains(...)` and make sure the dynamic semantics match.

#![feature(half_open_range_patterns)]
#![feature(exclusive_range_pattern)]
#![allow(illegal_floating_point_literal_pattern)]
#![allow(unreachable_patterns)]

macro_rules! yes {
    ($scrutinee:expr, $($t:tt)+) => {
        {
            let m = match $scrutinee { $($t)+ => true, _ => false, };
            let c = ($($t)+).contains(&$scrutinee);
            assert_eq!(m, c);
            m
        }
    }
}

fn range_to_inclusive() {
    // `..=X` (`RangeToInclusive`-equivalent):
    //---------------------------------------

    // u8; `..=X`
    assert!(yes!(u8::MIN, ..=u8::MIN));
    assert!(yes!(u8::MIN, ..=5));
    assert!(yes!(5u8, ..=5));
    assert!(!yes!(6u8, ..=5));

    // i16; `..=X`
    assert!(yes!(i16::MIN, ..=i16::MIN));
    assert!(yes!(i16::MIN, ..=0));
    assert!(yes!(i16::MIN, ..=-5));
    assert!(yes!(-5, ..=-5));
    assert!(!yes!(-4, ..=-5));

    // char; `..=X`
    assert!(yes!('\u{0}', ..='\u{0}'));
    assert!(yes!('\u{0}', ..='a'));
    assert!(yes!('a', ..='a'));
    assert!(!yes!('b', ..='a'));

    // f32; `..=X`
    assert!(yes!(f32::NEG_INFINITY, ..=f32::NEG_INFINITY));
    assert!(yes!(f32::NEG_INFINITY, ..=1.0f32));
    assert!(yes!(1.5f32, ..=1.5f32));
    assert!(!yes!(1.6f32, ..=-1.5f32));

    // f64; `..=X`
    assert!(yes!(f64::NEG_INFINITY, ..=f64::NEG_INFINITY));
    assert!(yes!(f64::NEG_INFINITY, ..=1.0f64));
    assert!(yes!(1.5f64, ..=1.5f64));
    assert!(!yes!(1.6f64, ..=-1.5f64));
}

fn range_to() {
    // `..X` (`RangeTo`-equivalent):
    //-----------------------------

    // u8; `..X`
    assert!(yes!(0u8, ..1));
    assert!(yes!(0u8, ..5));
    assert!(!yes!(5u8, ..5));
    assert!(!yes!(6u8, ..5));

    // u8; `..X`
    const NU8: u8 = u8::MIN + 1;
    assert!(yes!(u8::MIN, ..NU8));
    assert!(yes!(0u8, ..5));
    assert!(!yes!(5u8, ..5));
    assert!(!yes!(6u8, ..5));

    // i16; `..X`
    const NI16: i16 = i16::MIN + 1;
    assert!(yes!(i16::MIN, ..NI16));
    assert!(yes!(i16::MIN, ..5));
    assert!(yes!(-6, ..-5));
    assert!(!yes!(-5, ..-5));

    // char; `..X`
    assert!(yes!('\u{0}', ..'\u{1}'));
    assert!(yes!('\u{0}', ..'a'));
    assert!(yes!('a', ..'b'));
    assert!(!yes!('a', ..'a'));
    assert!(!yes!('b', ..'a'));

    // f32; `..X`
    assert!(yes!(f32::NEG_INFINITY, ..1.0f32));
    assert!(!yes!(1.5f32, ..1.5f32));
    const E32: f32 = 1.5f32 + f32::EPSILON;
    assert!(yes!(1.5f32, ..E32));
    assert!(!yes!(1.6f32, ..1.5f32));

    // f64; `..X`
    assert!(yes!(f64::NEG_INFINITY, ..1.0f64));
    assert!(!yes!(1.5f64, ..1.5f64));
    const E64: f64 = 1.5f64 + f64::EPSILON;
    assert!(yes!(1.5f64, ..E64));
    assert!(!yes!(1.6f64, ..1.5f64));
}

fn range_from() {
    // `X..` (`RangeFrom`-equivalent):
    //--------------------------------

    // u8; `X..`
    assert!(yes!(u8::MIN, u8::MIN..));
    assert!(yes!(u8::MAX, u8::MIN..));
    assert!(!yes!(u8::MIN, 1..));
    assert!(!yes!(4, 5..));
    assert!(yes!(5, 5..));
    assert!(yes!(6, 5..));
    assert!(yes!(u8::MAX, u8::MAX..));

    // i16; `X..`
    assert!(yes!(i16::MIN, i16::MIN..));
    assert!(yes!(i16::MAX, i16::MIN..));
    const NI16: i16 = i16::MIN + 1;
    assert!(!yes!(i16::MIN, NI16..));
    assert!(!yes!(-4, 5..));
    assert!(yes!(-4, -4..));
    assert!(yes!(-3, -4..));
    assert!(yes!(i16::MAX, i16::MAX..));

    // char; `X..`
    assert!(yes!('\u{0}', '\u{0}'..));
    assert!(yes!(core::char::MAX, '\u{0}'..));
    assert!(yes!('a', 'a'..));
    assert!(yes!('b', 'a'..));
    assert!(!yes!('a', 'b'..));
    assert!(yes!(core::char::MAX, core::char::MAX..));

    // f32; `X..`
    assert!(yes!(f32::NEG_INFINITY, f32::NEG_INFINITY..));
    assert!(yes!(f32::INFINITY, f32::NEG_INFINITY..));
    assert!(!yes!(f32::NEG_INFINITY, 1.0f32..));
    assert!(yes!(f32::INFINITY, 1.0f32..));
    assert!(!yes!(1.0f32 - f32::EPSILON, 1.0f32..));
    assert!(yes!(1.0f32, 1.0f32..));
    assert!(yes!(f32::INFINITY, 1.0f32..));
    assert!(yes!(f32::INFINITY, f32::INFINITY..));

    // f64; `X..`
    assert!(yes!(f64::NEG_INFINITY, f64::NEG_INFINITY..));
    assert!(yes!(f64::INFINITY, f64::NEG_INFINITY..));
    assert!(!yes!(f64::NEG_INFINITY, 1.0f64..));
    assert!(yes!(f64::INFINITY, 1.0f64..));
    assert!(!yes!(1.0f64 - f64::EPSILON, 1.0f64..));
    assert!(yes!(1.0f64, 1.0f64..));
    assert!(yes!(f64::INFINITY, 1.0f64..));
    assert!(yes!(f64::INFINITY, f64::INFINITY..));
}

fn main() {
    range_to_inclusive();
    range_to();
    range_from();
}
