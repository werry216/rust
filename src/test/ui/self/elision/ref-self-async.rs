// edition:2018

#![feature(async_await)]

#![feature(arbitrary_self_types)]
#![allow(non_snake_case)]

use std::marker::PhantomData;
use std::ops::Deref;
use std::pin::Pin;

struct Struct { }

struct Wrap<T, P>(T, PhantomData<P>);

impl<T, P> Deref for Wrap<T, P> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}

impl Struct {
    // Test using `&self` sugar:

    async fn ref_self(&self, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    // Test using `&Self` explicitly:

    async fn ref_Self(self: &Self, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    async fn box_ref_Self(self: Box<&Self>, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    async fn pin_ref_Self(self: Pin<&Self>, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    async fn box_box_ref_Self(self: Box<Box<&Self>>, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    async fn box_pin_ref_Self(self: Box<Pin<&Self>>, f: &u32) -> &u32 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }

    async fn wrap_ref_Self_Self(self: Wrap<&Self, Self>, f: &u8) -> &u8 {
        //~^ ERROR missing lifetime specifier
        //~| ERROR cannot infer an appropriate lifetime
        f
    }
}

fn main() { }
