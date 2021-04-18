// check-pass
#![warn(rustc::internal)]

#[allow(rustc::foo::bar::default_hash_types)]
//~^ WARN unknown lint: `rustc::foo::bar::default_hash_types`
//~| HELP did you mean
//~| SUGGESTION rustc::default_hash_types
#[allow(rustc::foo::default_hash_types)]
//~^ WARN unknown lint: `rustc::foo::default_hash_types`
//~| HELP did you mean
//~| SUGGESTION rustc::default_hash_types
fn main() {
    let _ = std::collections::HashMap::<String, String>::new();
    //~^ WARN Prefer FxHashMap over HashMap, it has better performance
    //~| HELP use
}
