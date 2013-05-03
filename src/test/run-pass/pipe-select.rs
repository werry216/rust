// xfail-fast

// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// xfail-pretty
// xfail-win32

extern mod std;
use std::timer::sleep;
use std::uv;

use core::cell::Cell;
use core::pipes;
use core::pipes::*;

proto! oneshot (
    waiting:send {
        signal -> !
    }
)

proto! stream (
    Stream:send<T:Owned> {
        send(T) -> Stream<T>
    }
)

pub fn spawn_service<T:Owned,Tb:Owned>(
            init: extern fn() -> (SendPacketBuffered<T, Tb>,
                                  RecvPacketBuffered<T, Tb>),
            service: ~fn(v: RecvPacketBuffered<T, Tb>))
        -> SendPacketBuffered<T, Tb> {
    let (client, server) = init();

    // This is some nasty gymnastics required to safely move the pipe
    // into a new task.
    let server = Cell(server);
    do task::spawn {
        service(server.take());
    }

    client
}

pub fn main() {
    use oneshot::client::*;
    use stream::client::*;

    let iotask = &uv::global_loop::get();

    let c = spawn_service(stream::init, |p| {
        error!("waiting for pipes");
        let stream::send(x, p) = recv(p);
        error!("got pipes");
        let (left, right) : (oneshot::server::waiting,
                             oneshot::server::waiting)
            = x;
        error!("selecting");
        let (i, _, _) = select(~[left, right]);
        error!("selected");
        assert!(i == 0);

        error!("waiting for pipes");
        let stream::send(x, _) = recv(p);
        error!("got pipes");
        let (left, right) : (oneshot::server::waiting,
                             oneshot::server::waiting)
            = x;
        error!("selecting");
        let (i, m, _) = select(~[left, right]);
        error!("selected %?", i);
        if m.is_some() {
            assert!(i == 1);
        }
    });

    let (c1, p1) = oneshot::init();
    let (_c2, p2) = oneshot::init();

    let c = send(c, (p1, p2));

    sleep(iotask, 100);

    signal(c1);

    let (_c1, p1) = oneshot::init();
    let (c2, p2) = oneshot::init();

    send(c, (p1, p2));

    sleep(iotask, 100);

    signal(c2);

    test_select2();
}

fn test_select2() {
    let (ac, ap) = stream::init();
    let (bc, bp) = stream::init();

    stream::client::send(ac, 42);

    match pipes::select2(ap, bp) {
      either::Left(*) => { }
      either::Right(*) => { fail!() }
    }

    stream::client::send(bc, ~"abc");

    error!("done with first select2");

    let (ac, ap) = stream::init();
    let (bc, bp) = stream::init();

    stream::client::send(bc, ~"abc");

    match pipes::select2(ap, bp) {
      either::Left(*) => { fail!() }
      either::Right(*) => { }
    }

    stream::client::send(ac, 42);
}
