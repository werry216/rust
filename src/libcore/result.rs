// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A type representing either success or failure

// NB: transitionary, de-mode-ing.

#[forbid(deprecated_mode)];
#[forbid(deprecated_pattern)];

use cmp::Eq;
use either::Either;

/// The result type
#[deriving_eq]
pub enum Result<T, U> {
    /// Contains the successful result value
    Ok(T),
    /// Contains the error value
    Err(U)
}

/**
 * Get the value out of a successful result
 *
 * # Failure
 *
 * If the result is an error
 */
pub pure fn get<T: Copy, U>(res: &Result<T, U>) -> T {
    match *res {
      Ok(copy t) => t,
      Err(ref the_err) => unsafe {
        fail fmt!("get called on error result: %?", *the_err)
      }
    }
}

/**
 * Get a reference to the value out of a successful result
 *
 * # Failure
 *
 * If the result is an error
 */
pub pure fn get_ref<T, U>(res: &a/Result<T, U>) -> &a/T {
    match *res {
        Ok(ref t) => t,
        Err(ref the_err) => unsafe {
            fail fmt!("get_ref called on error result: %?", *the_err)
        }
    }
}

/**
 * Get the value out of an error result
 *
 * # Failure
 *
 * If the result is not an error
 */
pub pure fn get_err<T, U: Copy>(res: &Result<T, U>) -> U {
    match *res {
      Err(copy u) => u,
      Ok(_) => fail ~"get_err called on ok result"
    }
}

/// Returns true if the result is `ok`
pub pure fn is_ok<T, U>(res: &Result<T, U>) -> bool {
    match *res {
      Ok(_) => true,
      Err(_) => false
    }
}

/// Returns true if the result is `err`
pub pure fn is_err<T, U>(res: &Result<T, U>) -> bool {
    !is_ok(res)
}

/**
 * Convert to the `either` type
 *
 * `ok` result variants are converted to `either::right` variants, `err`
 * result variants are converted to `either::left`.
 */
pub pure fn to_either<T: Copy, U: Copy>(res: &Result<U, T>)
    -> Either<T, U> {
    match *res {
      Ok(copy res) => either::Right(res),
      Err(copy fail_) => either::Left(fail_)
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `ok` then the value is extracted and passed to `op` whereupon
 * `op`s result is returned. if `res` is `err` then it is immediately
 * returned. This function can be used to compose the results of two
 * functions.
 *
 * Example:
 *
 *     let res = chain(read_file(file)) { |buf|
 *         ok(parse_bytes(buf))
 *     }
 */
pub pure fn chain<T, U, V>(res: Result<T, V>, op: fn(T)
    -> Result<U, V>) -> Result<U, V> {
    match move res {
        Ok(move t) => op(move t),
        Err(move e) => Err(move e)
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `err` then the value is extracted and passed to `op`
 * whereupon `op`s result is returned. if `res` is `ok` then it is
 * immediately returned.  This function can be used to pass through a
 * successful result while handling an error.
 */
pub pure fn chain_err<T, U, V>(
    res: Result<T, V>,
    op: fn(t: V) -> Result<T, U>)
    -> Result<T, U> {
    match move res {
      Ok(move t) => Ok(move t),
      Err(move v) => op(move v)
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `ok` then the value is extracted and passed to `op` whereupon
 * `op`s result is returned. if `res` is `err` then it is immediately
 * returned. This function can be used to compose the results of two
 * functions.
 *
 * Example:
 *
 *     iter(read_file(file)) { |buf|
 *         print_buf(buf)
 *     }
 */
pub pure fn iter<T, E>(res: &Result<T, E>, f: fn(&T)) {
    match *res {
      Ok(ref t) => f(t),
      Err(_) => ()
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `err` then the value is extracted and passed to `op` whereupon
 * `op`s result is returned. if `res` is `ok` then it is immediately returned.
 * This function can be used to pass through a successful result while
 * handling an error.
 */
pub pure fn iter_err<T, E>(res: &Result<T, E>, f: fn(&E)) {
    match *res {
      Ok(_) => (),
      Err(ref e) => f(e)
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `ok` then the value is extracted and passed to `op` whereupon
 * `op`s result is wrapped in `ok` and returned. if `res` is `err` then it is
 * immediately returned.  This function can be used to compose the results of
 * two functions.
 *
 * Example:
 *
 *     let res = map(read_file(file)) { |buf|
 *         parse_bytes(buf)
 *     }
 */
pub pure fn map<T, E: Copy, U: Copy>(res: &Result<T, E>, op: fn(&T) -> U)
  -> Result<U, E> {
    match *res {
      Ok(ref t) => Ok(op(t)),
      Err(copy e) => Err(e)
    }
}

/**
 * Call a function based on a previous result
 *
 * If `res` is `err` then the value is extracted and passed to `op` whereupon
 * `op`s result is wrapped in an `err` and returned. if `res` is `ok` then it
 * is immediately returned.  This function can be used to pass through a
 * successful result while handling an error.
 */
pub pure fn map_err<T: Copy, E, F: Copy>(res: &Result<T, E>, op: fn(&E) -> F)
  -> Result<T, F> {
    match *res {
      Ok(copy t) => Ok(t),
      Err(ref e) => Err(op(e))
    }
}

impl<T, E> Result<T, E> {
    #[inline(always)]
    pure fn get_ref(&self) -> &self/T { get_ref(self) }

    #[inline(always)]
    pure fn is_ok(&self) -> bool { is_ok(self) }

    #[inline(always)]
    pure fn is_err(&self) -> bool { is_err(self) }

    #[inline(always)]
    pure fn iter(&self, f: fn(&T)) { iter(self, f) }

    #[inline(always)]
    pure fn iter_err(&self, f: fn(&E)) { iter_err(self, f) }

    #[inline(always)]
    pure fn unwrap(self) -> T { unwrap(self) }

    #[inline(always)]
    pure fn unwrap_err(self) -> T { unwrap(self) }

    #[inline(always)]
    pure fn chain<U>(self, op: fn(T) -> Result<U,E>) -> Result<U,E> {
        chain(self, op)
    }

    #[inline(always)]
    pure fn chain_err<F>(self, op: fn(E) -> Result<T,F>) -> Result<T,F> {
        chain_err(self, op)
    }
}

impl<T: Copy, E> Result<T, E> {
    #[inline(always)]
    pure fn get(&self) -> T { get(self) }

    #[inline(always)]
    pure fn map_err<F:Copy>(&self, op: fn(&E) -> F) -> Result<T,F> {
        map_err(self, op)
    }
}

impl<T, E: Copy> Result<T, E> {
    #[inline(always)]
    pure fn get_err(&self) -> E { get_err(self) }

    #[inline(always)]
    pure fn map<U:Copy>(&self, op: fn(&T) -> U) -> Result<U,E> {
        map(self, op)
    }
}

/**
 * Maps each element in the vector `ts` using the operation `op`.  Should an
 * error occur, no further mappings are performed and the error is returned.
 * Should no error occur, a vector containing the result of each map is
 * returned.
 *
 * Here is an example which increments every integer in a vector,
 * checking for overflow:
 *
 *     fn inc_conditionally(x: uint) -> result<uint,str> {
 *         if x == uint::max_value { return err("overflow"); }
 *         else { return ok(x+1u); }
 *     }
 *     map(~[1u, 2u, 3u], inc_conditionally).chain {|incd|
 *         assert incd == ~[2u, 3u, 4u];
 *     }
 */
pub fn map_vec<T,U:Copy,V:Copy>(
    ts: &[T], op: fn(&T) -> Result<V,U>) -> Result<~[V],U> {

    let mut vs: ~[V] = vec::with_capacity(vec::len(ts));
    for vec::each(ts) |t| {
        match op(t) {
          Ok(copy v) => vs.push(v),
          Err(copy u) => return Err(u)
        }
    }
    return Ok(move vs);
}

pub fn map_opt<T,U:Copy,V:Copy>(
    o_t: &Option<T>, op: fn(&T) -> Result<V,U>) -> Result<Option<V>,U> {

    match *o_t {
      None => Ok(None),
      Some(ref t) => match op(t) {
        Ok(copy v) => Ok(Some(v)),
        Err(copy e) => Err(e)
      }
    }
}

/**
 * Same as map, but it operates over two parallel vectors.
 *
 * A precondition is used here to ensure that the vectors are the same
 * length.  While we do not often use preconditions in the standard
 * library, a precondition is used here because result::t is generally
 * used in 'careful' code contexts where it is both appropriate and easy
 * to accommodate an error like the vectors being of different lengths.
 */
pub fn map_vec2<S,T,U:Copy,V:Copy>(ss: &[S], ts: &[T],
                op: fn(&S,&T) -> Result<V,U>) -> Result<~[V],U> {

    assert vec::same_length(ss, ts);
    let n = vec::len(ts);
    let mut vs = vec::with_capacity(n);
    let mut i = 0u;
    while i < n {
        match op(&ss[i],&ts[i]) {
          Ok(copy v) => vs.push(v),
          Err(copy u) => return Err(u)
        }
        i += 1u;
    }
    return Ok(move vs);
}

/**
 * Applies op to the pairwise elements from `ss` and `ts`, aborting on
 * error.  This could be implemented using `map2()` but it is more efficient
 * on its own as no result vector is built.
 */
pub fn iter_vec2<S,T,U:Copy>(ss: &[S], ts: &[T],
                         op: fn(&S,&T) -> Result<(),U>) -> Result<(),U> {

    assert vec::same_length(ss, ts);
    let n = vec::len(ts);
    let mut i = 0u;
    while i < n {
        match op(&ss[i],&ts[i]) {
          Ok(()) => (),
          Err(copy u) => return Err(u)
        }
        i += 1u;
    }
    return Ok(());
}

/// Unwraps a result, assuming it is an `ok(T)`
#[inline(always)]
pub pure fn unwrap<T, U>(res: Result<T, U>) -> T {
    match move res {
      Ok(move t) => move t,
      Err(_) => fail ~"unwrap called on an err result"
    }
}

/// Unwraps a result, assuming it is an `err(U)`
#[inline(always)]
pub pure fn unwrap_err<T, U>(res: Result<T, U>) -> U {
    match move res {
      Err(move u) => move u,
      Ok(_) => fail ~"unwrap called on an ok result"
    }
}

#[cfg(test)]
#[allow(non_implicitly_copyable_typarams)]
mod tests {
    #[legacy_exports];
    fn op1() -> result::Result<int, ~str> { result::Ok(666) }

    fn op2(i: int) -> result::Result<uint, ~str> {
        result::Ok(i as uint + 1u)
    }

    fn op3() -> result::Result<int, ~str> { result::Err(~"sadface") }

    #[test]
    fn chain_success() {
        assert get(&chain(op1(), op2)) == 667u;
    }

    #[test]
    fn chain_failure() {
        assert get_err(&chain(op3(), op2)) == ~"sadface";
    }

    #[test]
    fn test_impl_iter() {
        let mut valid = false;
        Ok::<~str, ~str>(~"a").iter(|_x| valid = true);
        assert valid;

        Err::<~str, ~str>(~"b").iter(|_x| valid = false);
        assert valid;
    }

    #[test]
    fn test_impl_iter_err() {
        let mut valid = true;
        Ok::<~str, ~str>(~"a").iter_err(|_x| valid = false);
        assert valid;

        valid = false;
        Err::<~str, ~str>(~"b").iter_err(|_x| valid = true);
        assert valid;
    }

    #[test]
    fn test_impl_map() {
        assert Ok::<~str, ~str>(~"a").map(|_x| ~"b") == Ok(~"b");
        assert Err::<~str, ~str>(~"a").map(|_x| ~"b") == Err(~"a");
    }

    #[test]
    fn test_impl_map_err() {
        assert Ok::<~str, ~str>(~"a").map_err(|_x| ~"b") == Ok(~"a");
        assert Err::<~str, ~str>(~"a").map_err(|_x| ~"b") == Err(~"b");
    }

    #[test]
    fn test_get_ref_method() {
        let foo: Result<int, ()> = Ok(100);
        assert *foo.get_ref() == 100;
    }
}
