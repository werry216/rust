// run-pass

// edition:2018
// aux-build:arc_wake.rs

#![feature(async_await, async_closure, await_macro)]

extern crate arc_wake;

use std::pin::Pin;
use std::future::Future;
use std::sync::{
    Arc,
    atomic::{self, AtomicUsize},
};
use std::task::{Context, Poll};
use arc_wake::ArcWake;

struct Counter {
    wakes: AtomicUsize,
}

impl ArcWake for Counter {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self)
    }
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.wakes.fetch_add(1, atomic::Ordering::SeqCst);
    }
}

struct WakeOnceThenComplete(bool);

fn wake_and_yield_once() -> WakeOnceThenComplete { WakeOnceThenComplete(false) }

impl Future for WakeOnceThenComplete {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            self.0 = true;
            Poll::Pending
        }
    }
}

fn async_block(x: u8) -> impl Future<Output = u8> {
    async move {
        await!(wake_and_yield_once());
        x
    }
}

fn async_block_with_borrow_named_lifetime<'a>(x: &'a u8) -> impl Future<Output = u8> + 'a {
    async move {
        await!(wake_and_yield_once());
        *x
    }
}

fn async_nonmove_block(x: u8) -> impl Future<Output = u8> {
    async move {
        let future = async {
            await!(wake_and_yield_once());
            x
        };
        await!(future)
    }
}

fn async_closure(x: u8) -> impl Future<Output = u8> {
    (async move |x: u8| -> u8 {
        await!(wake_and_yield_once());
        x
    })(x)
}

async fn async_fn(x: u8) -> u8 {
    await!(wake_and_yield_once());
    x
}

async fn generic_async_fn<T>(x: T) -> T {
    await!(wake_and_yield_once());
    x
}

async fn async_fn_with_borrow(x: &u8) -> u8 {
    await!(wake_and_yield_once());
    *x
}

async fn async_fn_with_borrow_named_lifetime<'a>(x: &'a u8) -> u8 {
    await!(wake_and_yield_once());
    *x
}

fn async_fn_with_impl_future_named_lifetime<'a>(x: &'a u8) -> impl Future<Output = u8> + 'a {
    async move {
        await!(wake_and_yield_once());
        *x
    }
}

/* FIXME(cramertj) support when `existential type T<'a, 'b>:;` works
async fn async_fn_multiple_args(x: &u8, _y: &u8) -> u8 {
    await!(wake_and_yield_once());
    *x
}
*/

async fn async_fn_multiple_args_named_lifetime<'a>(x: &'a u8, _y: &'a u8) -> u8 {
    await!(wake_and_yield_once());
    *x
}

fn async_fn_with_internal_borrow(y: u8) -> impl Future<Output = u8> {
    async move {
        await!(async_fn_with_borrow_named_lifetime(&y))
    }
}

async unsafe fn unsafe_async_fn(x: u8) -> u8 {
    await!(wake_and_yield_once());
    x
}

struct Foo;

trait Bar {
    fn foo() {}
}

impl Foo {
    async fn async_method(x: u8) -> u8 {
        unsafe {
            await!(unsafe_async_fn(x))
        }
    }
}

fn test_future_yields_once_then_returns<F, Fut>(f: F)
where
    F: FnOnce(u8) -> Fut,
    Fut: Future<Output = u8>,
{
    let mut fut = Box::pin(f(9));
    let counter = Arc::new(Counter { wakes: AtomicUsize::new(0) });
    let waker = ArcWake::into_waker(counter.clone());
    let mut cx = Context::from_waker(&waker);
    assert_eq!(0, counter.wakes.load(atomic::Ordering::SeqCst));
    assert_eq!(Poll::Pending, fut.as_mut().poll(&mut cx));
    assert_eq!(1, counter.wakes.load(atomic::Ordering::SeqCst));
    assert_eq!(Poll::Ready(9), fut.as_mut().poll(&mut cx));
}

fn main() {
    macro_rules! test {
        ($($fn_name:expr,)*) => { $(
            test_future_yields_once_then_returns($fn_name);
        )* }
    }

    macro_rules! test_with_borrow {
        ($($fn_name:expr,)*) => { $(
            test_future_yields_once_then_returns(|x| {
                async move {
                    await!($fn_name(&x))
                }
            });
        )* }
    }

    test! {
        async_block,
        async_nonmove_block,
        async_closure,
        async_fn,
        generic_async_fn,
        async_fn_with_internal_borrow,
        Foo::async_method,
        |x| {
            async move {
                unsafe { await!(unsafe_async_fn(x)) }
            }
        },
    }
    test_with_borrow! {
        async_block_with_borrow_named_lifetime,
        async_fn_with_borrow,
        async_fn_with_borrow_named_lifetime,
        async_fn_with_impl_future_named_lifetime,
        |x| {
            async move {
                await!(async_fn_multiple_args_named_lifetime(x, x))
            }
        },
    }
}
