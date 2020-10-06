// check-pass
#![feature(c_variadic)]
#![warn(function_item_references)]
use std::fmt::Pointer;

fn nop() { }
fn foo() -> u32 { 42 }
fn bar(x: u32) -> u32 { x }
fn baz(x: u32, y: u32) -> u32 { x + y }
unsafe fn unsafe_fn() { }
extern "C" fn c_fn() { }
unsafe extern "C" fn unsafe_c_fn() { }
unsafe extern fn variadic_fn(_x: u32, _args: ...) { }

//function references passed to these functions should never lint
fn call_fn(f: &dyn Fn(u32) -> u32, x: u32) { f(x); }
fn parameterized_call_fn<F: Fn(u32) -> u32>(f: &F, x: u32) { f(x); }

//function references passed to these functions should lint
fn print_ptr<F: Pointer>(f: F) { println!("{:p}", f); }
fn bound_by_ptr_trait<F: Pointer>(_f: F) { }
fn bound_by_ptr_trait_tuple<F: Pointer, G: Pointer>(_t: (F, G)) { }
fn implicit_ptr_trait<F>(f: &F) { println!("{:p}", f); }

fn main() {
    //`let` bindings with function references shouldn't lint
    let _ = &foo;
    let _ = &mut foo;

    let zst_ref = &foo;
    let fn_item = foo;
    let indirect_ref = &fn_item;

    let _mut_zst_ref = &mut foo;
    let mut mut_fn_item = foo;
    let _mut_indirect_ref = &mut mut_fn_item;

    let cast_zst_ptr = &foo as *const _;
    let coerced_zst_ptr: *const _ = &foo;

    let _mut_cast_zst_ptr = &mut foo as *mut _;
    let _mut_coerced_zst_ptr: *mut _ = &mut foo;

    let _cast_zst_ref = &foo as &dyn Fn() -> u32;
    let _coerced_zst_ref: &dyn Fn() -> u32 = &foo;

    let _mut_cast_zst_ref = &mut foo as &mut dyn Fn() -> u32;
    let _mut_coerced_zst_ref: &mut dyn Fn() -> u32 = &mut foo;

    //the suggested way to cast to a function pointer
    let fn_ptr = foo as fn() -> u32;

    //correct ways to print function pointers
    println!("{:p}", foo as fn() -> u32);
    println!("{:p}", fn_ptr);

    //potential ways to incorrectly try printing function pointers
    println!("{:p}", &foo);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    print!("{:p}", &foo);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    format!("{:p}", &foo);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer

    println!("{:p}", &foo as *const _);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    println!("{:p}", zst_ref);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    println!("{:p}", cast_zst_ptr);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    println!("{:p}", coerced_zst_ptr);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer

    println!("{:p}", &fn_item);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    println!("{:p}", indirect_ref);
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer

    println!("{:p}", &nop);
    //~^ WARNING cast `nop` with `as fn()` to use it as a pointer
    println!("{:p}", &bar);
    //~^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer
    println!("{:p}", &baz);
    //~^ WARNING cast `baz` with `as fn(_, _) -> _` to use it as a pointer
    println!("{:p}", &unsafe_fn);
    //~^ WARNING cast `unsafe_fn` with `as unsafe fn()` to use it as a pointer
    println!("{:p}", &c_fn);
    //~^ WARNING cast `c_fn` with `as extern "C" fn()` to use it as a pointer
    println!("{:p}", &unsafe_c_fn);
    //~^ WARNING cast `unsafe_c_fn` with `as unsafe extern "C" fn()` to use it as a pointer
    println!("{:p}", &variadic_fn);
    //~^ WARNING cast `variadic_fn` with `as unsafe extern "C" fn(_, ...)` to use it as a pointer
    println!("{:p}", &std::env::var::<String>);
    //~^ WARNING cast `var` with `as fn(_) -> _` to use it as a pointer

    println!("{:p} {:p} {:p}", &nop, &foo, &bar);
    //~^ WARNING cast `nop` with `as fn()` to use it as a pointer
    //~^^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    //~^^^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer

    //using a function reference to call a function shouldn't lint
    (&bar)(1);

    //passing a function reference to an arbitrary function shouldn't lint
    call_fn(&bar, 1);
    parameterized_call_fn(&bar, 1);
    std::mem::size_of_val(&foo);

    unsafe {
        //potential ways to incorrectly try transmuting function pointers
        std::mem::transmute::<_, usize>(&foo);
        //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
        std::mem::transmute::<_, (usize, usize)>((&foo, &bar));
        //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
        //~^^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer

        //the correct way to transmute function pointers
        std::mem::transmute::<_, usize>(foo as fn() -> u32);
        std::mem::transmute::<_, (usize, usize)>((foo as fn() -> u32, bar as fn(u32) -> u32));
    }

    //function references as arguments required to be bound by std::fmt::Pointer should lint
    print_ptr(&bar);
    //~^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer
    bound_by_ptr_trait(&bar);
    //~^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer
    bound_by_ptr_trait_tuple((&foo, &bar));
    //~^ WARNING cast `foo` with `as fn() -> _` to use it as a pointer
    //~^^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer
    implicit_ptr_trait(&bar);
    //~^ WARNING cast `bar` with `as fn(_) -> _` to use it as a pointer

    //correct ways to pass function pointers as arguments bound by std::fmt::Pointer
    print_ptr(bar as fn(u32) -> u32);
    bound_by_ptr_trait(bar as fn(u32) -> u32);
    bound_by_ptr_trait_tuple((foo as fn() -> u32, bar as fn(u32) -> u32));
}
