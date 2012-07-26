/// An interface for numbers.

trait num {
    // FIXME: Cross-crate overloading doesn't work yet. (#2615)
    // FIXME: Interface inheritance. (#2616)
    pure fn add(&&other: self) -> self;
    pure fn sub(&&other: self) -> self;
    pure fn mul(&&other: self) -> self;
    pure fn div(&&other: self) -> self;
    pure fn modulo(&&other: self) -> self;
    pure fn neg() -> self;

    pure fn to_int() -> int;
    pure fn from_int(n: int) -> self;    // FIXME (#2376) Static functions.
    // n.b. #2376 is for classes, not ifaces, but it could be generalized...
}

