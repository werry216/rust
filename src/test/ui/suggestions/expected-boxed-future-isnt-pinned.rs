// edition:2018
// run-rustfix
#![allow(dead_code)]
use std::future::Future;
use std::pin::Pin;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
//   ^^^^^^^^^ This would come from the `futures` crate in real code.

fn foo() -> BoxFuture<'static, i32> {
    async { //~ ERROR mismatched types
        42
    }
}

fn main() {}
