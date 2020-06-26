#!/usr/bin/env python3

"""
Generate powers of ten using William Clinger's ``AlgorithmM`` for use in
decimal to floating point conversions.

Specifically, computes and outputs (as Rust code) a table of 10^e for some
range of exponents e. The output is one array of 64 bit significands and
another array of corresponding base two exponents. The approximations are
normalized and rounded perfectly, i.e., within 0.5 ULP of the true value.

The representation ([u64], [i16]) instead of the more natural [(u64, i16)]
is used because (u64, i16) has a ton of padding which would make the table
even larger, and it's already uncomfortably large (6 KiB).
"""
from __future__ import print_function
from math import ceil, log
from fractions import Fraction
from collections import namedtuple


N = 64  # Size of the significand field in bits
MIN_SIG = 2 ** (N - 1)
MAX_SIG = (2 ** N) - 1

# Hand-rolled fp representation without arithmetic or any other operations.
# The significand is normalized and always N bit, but the exponent is
# unrestricted in range.
Fp = namedtuple('Fp', 'sig exp')


def algorithm_m(f, e):
    assert f > 0
    if e < 0:
        u = f
        v = 10 ** abs(e)
    else:
        u = f * 10 ** e
        v = 1
    k = 0
    x = u // v
    while True:
        if x < MIN_SIG:
            u <<= 1
            k -= 1
        elif x >= MAX_SIG:
            v <<= 1
            k += 1
        else:
            break
        x = u // v
    return ratio_to_float(u, v, k)


def ratio_to_float(u, v, k):
    q, r = divmod(u, v)
    v_r = v - r
    z = Fp(q, k)
    if r < v_r:
        return z
    elif r > v_r:
        return next_float(z)
    elif q % 2 == 0:
        return z
    else:
        return next_float(z)


def next_float(z):
    if z.sig == MAX_SIG:
        return Fp(MIN_SIG, z.exp + 1)
    else:
        return Fp(z.sig + 1, z.exp)


def error(f, e, z):
    decimal = f * Fraction(10) ** e
    binary = z.sig * Fraction(2) ** z.exp
    abs_err = abs(decimal - binary)
    # The unit in the last place has value z.exp
    ulp_err = abs_err / Fraction(2) ** z.exp
    return float(ulp_err)


HEADER = """
//! Tables of approximations of powers of ten.
//! DO NOT MODIFY: Generated by `src/etc/dec2flt_table.py`
"""


def main():
    print(HEADER.strip())
    print()
    print_proper_powers()
    print()
    print_short_powers(32, 24)
    print()
    print_short_powers(64, 53)


def print_proper_powers():
    MIN_E = -305
    MAX_E = 305
    e_range = range(MIN_E, MAX_E+1)
    powers = []
    for e in e_range:
        z = algorithm_m(1, e)
        err = error(1, e, z)
        assert err < 0.5
        powers.append(z)
    print("pub const MIN_E: i16 = {};".format(MIN_E))
    print("pub const MAX_E: i16 = {};".format(MAX_E))
    print()
    print("#[rustfmt::skip]")
    typ = "([u64; {0}], [i16; {0}])".format(len(powers))
    print("pub const POWERS: ", typ, " = (", sep='')
    print("    [")
    for z in powers:
        print("        0x{:x},".format(z.sig))
    print("    ],")
    print("    [")
    for z in powers:
        print("        {},".format(z.exp))
    print("    ],")
    print(");")


def print_short_powers(num_bits, significand_size):
    max_sig = 2**significand_size - 1
    # The fast path bails out for exponents >= ceil(log5(max_sig))
    max_e = int(ceil(log(max_sig, 5)))
    e_range = range(max_e)
    typ = "[f{}; {}]".format(num_bits, len(e_range))
    print("#[rustfmt::skip]")
    print("pub const F", num_bits, "_SHORT_POWERS: ", typ, " = [", sep='')
    for e in e_range:
        print("    1e{},".format(e))
    print("];")


if __name__ == '__main__':
    main()
