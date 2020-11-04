#[test]
fn test_nan() {
    use core::f64;
    let x = "NaN".to_string();
    assert_eq!(format!("{}", f64::NAN), x);
    assert_eq!(format!("{:e}", f64::NAN), x);
    assert_eq!(format!("{:E}", f64::NAN), x);
}
