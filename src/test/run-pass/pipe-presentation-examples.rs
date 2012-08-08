// Examples from Eric's internship final presentation.
//
// Code is easier to write in emacs, and it's good to be sure all the
// code samples compile (or not) as they should.

// xfail-pretty

import double_buffer::client::*;
import double_buffer::give_buffer;

macro_rules! select_if {
    {
        $index:expr,
        $count:expr,
        $port:path => [
            $($message:path$(($($x: ident),+))dont_type_this*
              -> $next:ident $e:expr),+
        ],
        $( $ports:path => [
            $($messages:path$(($($xs: ident),+))dont_type_this*
              -> $nexts:ident $es:expr),+
        ], )*
    } => {
        log_syntax!{select_if1};
        if $index == $count {
            match move pipes::try_recv($port) {
              $(some($message($($(copy $x,)+)* next)) => {
                // FIXME (#2329) we really want move out of enum here.
                let $next = unsafe { let x <- *ptr::addr_of(next); x };
                $e
              })+
              _ => fail
            }
        } else {
            select_if!{
                $index,
                $count + 1,
                $( $ports => [
                    $($messages$(($($xs),+))dont_type_this*
                      -> $nexts $es),+
                ], )*
            }
        }
    };

    {
        $index:expr,
        $count:expr,
    } => {
        log_syntax!{select_if2};
        fail
    }
}

macro_rules! select {
    {
        $( $port:path => {
            $($message:path$(($($x: ident),+))dont_type_this*
              -> $next:ident $e:expr),+
        } )+
    } => {
        let index = pipes::selecti([$(($port).header()),+]/_);
        log_syntax!{select};
        log_syntax!{
        select_if!{index, 0, $( $port => [
            $($message$(($($x),+))dont_type_this* -> $next $e),+
        ], )+}
        };
        select_if!{index, 0, $( $port => [
            $($message$(($($x),+))dont_type_this* -> $next $e),+
        ], )+}
    }
}

// Types and protocols
struct Buffer {
    foo: ();

    drop { }
}

proto! double_buffer {
    acquire:send {
        request -> wait_buffer
    }

    wait_buffer:recv {
        give_buffer(Buffer) -> release
    }

    release:send {
        release(Buffer) -> acquire
    }
}

// Code examples
fn render(_buffer: &Buffer) {
    // A dummy function.
}

fn draw_frame(+channel: double_buffer::client::acquire) {
    let channel = request(channel);
    select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                release(channel, move buffer)
            }
        }
    };
}

fn draw_two_frames(+channel: double_buffer::client::acquire) {
    let channel = request(channel);
    let channel = select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                release(channel, move buffer)
            }
        }
    };
    let channel = request(channel);
    select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                release(channel, move buffer)
            }
        }
    };
}

#[cfg(bad1)]
fn draw_two_frames_bad1(+channel: double_buffer::client::acquire) {
    let channel = request(channel);
    let channel = select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                channel
            }
        }
    };
    let channel = request(channel);
    select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                release(channel, move buffer)
            }
        }
    };
}

#[cfg(bad2)]
fn draw_two_frames_bad2(+channel: double_buffer::client::acquire) {
    let channel = request(channel);
    select! {
        channel => {
            give_buffer(buffer) -> channel {
                render(&buffer);
                release(channel, move buffer);
                render(&buffer);
                release(channel, move buffer);
            }
        }
    };
}

fn main() { }