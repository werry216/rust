// compile-flags: -Z parse-only

pub extern
  "invalid-ab_isize" //~ ERROR invalid ABI
fn foo() {}

fn main() {}
