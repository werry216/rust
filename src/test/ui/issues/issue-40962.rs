// build-pass (FIXME(62277): could be check-pass?)
macro_rules! m {
    ($i:meta) => {
        #[derive($i)]
        struct S;
    }
}

m!(Clone);

fn main() {}
