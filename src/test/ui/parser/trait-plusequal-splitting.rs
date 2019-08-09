// Fixes issue where `+` in generics weren't parsed if they were part of a `+=`.

// build-pass (FIXME(62277): could be check-pass?)

struct Whitespace<T: Clone + = ()> { t: T }
struct TokenSplit<T: Clone +=  ()> { t: T }

fn main() {}
