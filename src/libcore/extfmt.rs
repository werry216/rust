// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Support for fmt! expressions.
//!
//! The syntax is close to that of Posix format strings:
//!
//! ~~~~~~
//! Format := '%' Parameter? Flag* Width? Precision? Type
//! Parameter := [0-9]+ '$'
//! Flag := [ 0#+-]
//! Width := Parameter | [0-9]+
//! Precision := '.' [0-9]+
//! Type := [bcdfiostuxX?]
//! ~~~~~~
//!
//! * Parameter is the 1-based argument to apply the format to. Currently not
//! implemented.
//! * Flag 0 causes leading zeros to be used for padding when converting
//! numbers.
//! * Flag # causes the conversion to be done in an *alternative* manner.
//! Currently not implemented.
//! * Flag + causes signed numbers to always be prepended with a sign
//! character.
//! * Flag - left justifies the result
//! * Width specifies the minimum field width of the result. By default
//! leading spaces are added.
//! * Precision specifies the minimum number of digits for integral types
//! and the minimum number
//! of decimal places for float.
//!
//! The types currently supported are:
//!
//! * b - bool
//! * c - char
//! * d - int
//! * f - float
//! * i - int (same as d)
//! * o - uint as octal
//! * t - uint as binary
//! * u - uint
//! * x - uint as lower-case hexadecimal
//! * X - uint as upper-case hexadecimal
//! * s - str (any flavor)
//! * ? - arbitrary type (does not use the to_str trait)

// NB: transitionary, de-mode-ing.
#[forbid(deprecated_mode)];
#[forbid(deprecated_pattern)];

/*
Syntax Extension: fmt

Format a string

The 'fmt' extension is modeled on the posix printf system.

A posix conversion ostensibly looks like this

> %~[parameter]~[flags]~[width]~[.precision]~[length]type

Given the different numeric type bestiary we have, we omit the 'length'
parameter and support slightly different conversions for 'type'

> %~[parameter]~[flags]~[width]~[.precision]type

we also only support translating-to-rust a tiny subset of the possible
combinations at the moment.

Example:

debug!("hello, %s!", "world");

*/

use cmp::Eq;
use option::{Some, None};
use str;

/*
 * We have a 'ct' (compile-time) module that parses format strings into a
 * sequence of conversions. From those conversions AST fragments are built
 * that call into properly-typed functions in the 'rt' (run-time) module.
 * Each of those run-time conversion functions accepts another conversion
 * description that specifies how to format its output.
 *
 * The building of the AST is currently done in a module inside the compiler,
 * but should migrate over here as the plugin interface is defined.
 */

// Functions used by the fmt extension at compile time
#[doc(hidden)]
pub mod ct {
    use char;
    use str;
    use vec;

    pub enum Signedness { Signed, Unsigned, }
    pub enum Caseness { CaseUpper, CaseLower, }
    pub enum Ty {
        TyBool,
        TyStr,
        TyChar,
        TyInt(Signedness),
        TyBits,
        TyHex(Caseness),
        TyOctal,
        TyFloat,
        TyPoly,
    }
    pub enum Flag {
        FlagLeftJustify,
        FlagLeftZeroPad,
        FlagSpaceForSign,
        FlagSignAlways,
        FlagAlternate,
    }
    pub enum Count {
        CountIs(uint),
        CountIsParam(uint),
        CountIsNextParam,
        CountImplied,
    }

    struct Parsed<T> {
        val: T,
        next: uint
    }

    impl<T> Parsed<T> {
        static pure fn new(val: T, next: uint) -> Parsed<T> {
            Parsed { val: val, next: next }
        }
    }

    // A formatted conversion from an expression to a string
    pub struct Conv
        {param: Option<uint>,
         flags: ~[Flag],
         width: Count,
         precision: Count,
         ty: Ty}


    // A fragment of the output sequence
    pub enum Piece { PieceString(~str), PieceConv(Conv), }
    pub type ErrorFn = fn@(&str) -> ! ;

    pub fn parse_fmt_string(s: &str, err: ErrorFn) -> ~[Piece] {
        let mut pieces: ~[Piece] = ~[];
        let lim = str::len(s);
        let mut buf = ~"";
        fn flush_buf(buf: ~str, pieces: &mut ~[Piece]) -> ~str {
            if buf.len() > 0 {
                let piece = PieceString(move buf);
                pieces.push(move piece);
            }
            return ~"";
        }
        let mut i = 0;
        while i < lim {
            let size = str::utf8_char_width(s[i]);
            let curr = str::slice(s, i, i+size);
            if curr == ~"%" {
                i += 1;
                if i >= lim {
                    err(~"unterminated conversion at end of string");
                }
                let curr2 = str::slice(s, i, i+1);
                if curr2 == ~"%" {
                    buf += curr2;
                    i += 1;
                } else {
                    buf = flush_buf(move buf, &mut pieces);
                    let rs = parse_conversion(s, i, lim, err);
                    pieces.push(copy rs.val);
                    i = rs.next;
                }
            } else { buf += curr; i += size; }
        }
        flush_buf(move buf, &mut pieces);
        move pieces
    }
    pub fn peek_num(s: &str, i: uint, lim: uint) ->
       Option<Parsed<uint>> {
        let mut j = i;
        let mut accum = 0u;
        let mut found = false;
        while j < lim {
            match char::to_digit(s[j] as char, 10) {
                Some(x) => {
                    found = true;
                    accum *= 10;
                    accum += x;
                    j += 1;
                },
                None => break
            }
        }
        if found {
            Some(Parsed::new(accum, j))
        } else {
            None
        }
    }
    pub fn parse_conversion(s: &str, i: uint, lim: uint,
                            err: ErrorFn) ->
       Parsed<Piece> {
        let parm = parse_parameter(s, i, lim);
        let flags = parse_flags(s, parm.next, lim);
        let width = parse_count(s, flags.next, lim);
        let prec = parse_precision(s, width.next, lim);
        let ty = parse_type(s, prec.next, lim, err);
        return Parsed::new(
                 PieceConv(Conv {param: parm.val,
                             flags: copy flags.val,
                             width: width.val,
                             precision: prec.val,
                             ty: ty.val}),
             ty.next);
    }
    pub fn parse_parameter(s: &str, i: uint, lim: uint) ->
       Parsed<Option<uint>> {
        if i >= lim { return Parsed::new(None, i); }
        let num = peek_num(s, i, lim);
        return match num {
              None => Parsed::new(None, i),
              Some(t) => {
                let n = t.val;
                let j = t.next;
                if j < lim && s[j] == '$' as u8 {
                    Parsed::new(Some(n), j + 1)
                } else { Parsed::new(None, i) }
              }
            };
    }
    pub fn parse_flags(s: &str, i: uint, lim: uint) ->
       Parsed<~[Flag]> {
        let mut i = i;
        let mut flags = ~[];

        while i < lim {
            let f = match s[i] {
                '-' as u8 => FlagLeftJustify,
                '0' as u8 => FlagLeftZeroPad,
                ' ' as u8 => FlagSpaceForSign,
                '+' as u8 => FlagSignAlways,
                '#' as u8 => FlagAlternate,
                _ => break
            };

            flags.push(f);
            i += 1;
        }

        Parsed::new(flags, i)
    }
        pub fn parse_count(s: &str, i: uint, lim: uint)
        -> Parsed<Count> {
            if i >= lim {
                Parsed::new(CountImplied, i)
            } else if s[i] == '*' as u8 {
                let param = parse_parameter(s, i + 1, lim);
                let j = param.next;
                match param.val {
                  None => Parsed::new(CountIsNextParam, j),
                  Some(n) => Parsed::new(CountIsParam(n), j)
                }
            } else {
                match peek_num(s, i, lim) {
                  None => Parsed::new(CountImplied, i),
                  Some(num) => Parsed::new(
                    CountIs(num.val),
                    num.next
                  )
                }
            }
    }
    pub fn parse_precision(s: &str, i: uint, lim: uint) ->
       Parsed<Count> {
            if i < lim && s[i] == '.' as u8 {
                let count = parse_count(s, i + 1u, lim);


                // If there were no digits specified, i.e. the precision
                // was ".", then the precision is 0
                match count.val {
                  CountImplied => Parsed::new(CountIs(0), count.next),
                  _ => count
                }
            } else { Parsed::new(CountImplied, i) }
    }
    pub fn parse_type(s: &str, i: uint, lim: uint, err: ErrorFn) ->
       Parsed<Ty> {
        if i >= lim { err(~"missing type in conversion"); }
        // FIXME (#2249): Do we really want two signed types here?
        // How important is it to be printf compatible?
        let t = match s[i] {
            'b' as u8 => TyBool,
            's' as u8 => TyStr,
            'c' as u8 => TyChar,
            'd' as u8 | 'i' as u8 => TyInt(Signed),
            'u' as u8 => TyInt(Unsigned),
            'x' as u8 => TyHex(CaseLower),
            'X' as u8 => TyHex(CaseUpper),
            't' as u8 => TyBits,
            'o' as u8 => TyOctal,
            'f' as u8 => TyFloat,
            '?' as u8 => TyPoly,
            _ => err(~"unknown type in conversion: " + s.substr(i, 1))
        };
        Parsed::new(t, i + 1)
    }
}

// Functions used by the fmt extension at runtime. For now there are a lot of
// decisions made a runtime. If it proves worthwhile then some of these
// conditions can be evaluated at compile-time. For now though it's cleaner to
// implement it 0this way, I think.
#[doc(hidden)]
pub mod rt {
    use float;
    use str;
    use sys;
    use uint;
    use vec;

    pub const flag_none : u32 = 0u32;
    pub const flag_left_justify   : u32 = 0b00000000000001u32;
    pub const flag_left_zero_pad  : u32 = 0b00000000000010u32;
    pub const flag_space_for_sign : u32 = 0b00000000000100u32;
    pub const flag_sign_always    : u32 = 0b00000000001000u32;
    pub const flag_alternate      : u32 = 0b00000000010000u32;

    pub enum Count { CountIs(uint), CountImplied, }

    pub enum Ty { TyDefault, TyBits, TyHexUpper, TyHexLower, TyOctal, }

    pub type Conv = {flags: u32, width: Count, precision: Count, ty: Ty};

    pub pure fn conv_int(cv: Conv, i: int) -> ~str {
        let radix = 10;
        let prec = get_int_precision(cv);
        let mut s : ~str = int_to_str_prec(i, radix, prec);
        if 0 <= i {
            if have_flag(cv.flags, flag_sign_always) {
                unsafe { str::unshift_char(&mut s, '+') };
            } else if have_flag(cv.flags, flag_space_for_sign) {
                unsafe { str::unshift_char(&mut s, ' ') };
            }
        }
        return unsafe { pad(cv, move s, PadSigned) };
    }
    pub pure fn conv_uint(cv: Conv, u: uint) -> ~str {
        let prec = get_int_precision(cv);
        let mut rs =
            match cv.ty {
              TyDefault => uint_to_str_prec(u, 10, prec),
              TyHexLower => uint_to_str_prec(u, 16, prec),
              TyHexUpper => str::to_upper(uint_to_str_prec(u, 16, prec)),
              TyBits => uint_to_str_prec(u, 2, prec),
              TyOctal => uint_to_str_prec(u, 8, prec)
            };
        return unsafe { pad(cv, move rs, PadUnsigned) };
    }
    pub pure fn conv_bool(cv: Conv, b: bool) -> ~str {
        let s = if b { ~"true" } else { ~"false" };
        // run the boolean conversion through the string conversion logic,
        // giving it the same rules for precision, etc.
        return conv_str(cv, s);
    }
    pub pure fn conv_char(cv: Conv, c: char) -> ~str {
        let mut s = str::from_char(c);
        return unsafe { pad(cv, move s, PadNozero) };
    }
    pub pure fn conv_str(cv: Conv, s: &str) -> ~str {
        // For strings, precision is the maximum characters
        // displayed
        let mut unpadded = match cv.precision {
          CountImplied => s.to_owned(),
          CountIs(max) => if max as uint < str::char_len(s) {
            str::substr(s, 0, max as uint)
          } else {
            s.to_owned()
          }
        };
        return unsafe { pad(cv, move unpadded, PadNozero) };
    }
    pub pure fn conv_float(cv: Conv, f: float) -> ~str {
        let (to_str, digits) = match cv.precision {
              CountIs(c) => (float::to_str_exact, c as uint),
              CountImplied => (float::to_str, 6u)
        };
        let mut s = unsafe { to_str(f, digits) };
        if 0.0 <= f {
            if have_flag(cv.flags, flag_sign_always) {
                s = ~"+" + s;
            } else if have_flag(cv.flags, flag_space_for_sign) {
                s = ~" " + s;
            }
        }
        return unsafe { pad(cv, move s, PadFloat) };
    }
    pub pure fn conv_poly<T>(cv: Conv, v: &T) -> ~str {
        let s = sys::log_str(v);
        return conv_str(cv, s);
    }

    // Convert an int to string with minimum number of digits. If precision is
    // 0 and num is 0 then the result is the empty string.
    pub pure fn int_to_str_prec(num: int, radix: uint, prec: uint) -> ~str {
        return if num < 0 {
                ~"-" + uint_to_str_prec(-num as uint, radix, prec)
            } else { uint_to_str_prec(num as uint, radix, prec) };
    }

    // Convert a uint to string with a minimum number of digits.  If precision
    // is 0 and num is 0 then the result is the empty string. Could move this
    // to uint: but it doesn't seem all that useful.
    pub pure fn uint_to_str_prec(num: uint, radix: uint,
                                 prec: uint) -> ~str {
        return if prec == 0u && num == 0u {
                ~""
            } else {
                let s = uint::to_str(num, radix);
                let len = str::char_len(s);
                if len < prec {
                    let diff = prec - len;
                    let pad = str::from_chars(vec::from_elem(diff, '0'));
                    pad + s
                } else { move s }
            };
    }
    pub pure fn get_int_precision(cv: Conv) -> uint {
        return match cv.precision {
              CountIs(c) => c as uint,
              CountImplied => 1u
            };
    }

    #[deriving_eq]
    pub enum PadMode { PadSigned, PadUnsigned, PadNozero, PadFloat }

    pub fn pad(cv: Conv, s: ~str, mode: PadMode) -> ~str {
        let mut s = move s; // sadtimes
        let uwidth : uint = match cv.width {
          CountImplied => return (move s),
          CountIs(width) => { width as uint }
        };
        let strlen = str::char_len(s);
        if uwidth <= strlen { return (move s); }
        let mut padchar = ' ';
        let diff = uwidth - strlen;
        if have_flag(cv.flags, flag_left_justify) {
            let padstr = str::from_chars(vec::from_elem(diff, padchar));
            return s + padstr;
        }
        let {might_zero_pad, signed} = match mode {
          PadNozero => {might_zero_pad:false, signed:false},
          PadSigned => {might_zero_pad:true,  signed:true },
          PadFloat => {might_zero_pad:true,  signed:true},
          PadUnsigned => {might_zero_pad:true,  signed:false}
        };
        pure fn have_precision(cv: Conv) -> bool {
            return match cv.precision { CountImplied => false, _ => true };
        }
        let zero_padding = {
            if might_zero_pad && have_flag(cv.flags, flag_left_zero_pad) &&
                (!have_precision(cv) || mode == PadFloat) {
                padchar = '0';
                true
            } else {
                false
            }
        };
        let padstr = str::from_chars(vec::from_elem(diff, padchar));
        // This is completely heinous. If we have a signed value then
        // potentially rip apart the intermediate result and insert some
        // zeros. It may make sense to convert zero padding to a precision
        // instead.

        if signed && zero_padding && s.len() > 0 {
            let head = str::shift_char(&mut s);
            if head == '+' || head == '-' || head == ' ' {
                let headstr = str::from_chars(vec::from_elem(1u, head));
                return headstr + padstr + s;
            }
            else {
                str::unshift_char(&mut s, head);
            }
        }
        return padstr + s;
    }
    pub pure fn have_flag(flags: u32, f: u32) -> bool {
        flags & f != 0
    }
}

// Bulk of the tests are in src/test/run-pass/syntax-extension-fmt.rs
#[cfg(test)]
mod test {
    #[test]
    fn fmt_slice() {
        let s = "abc";
        let _s = fmt!("%s", s);
    }
}

// Local Variables:
// mode: rust;
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
