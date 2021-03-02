//! # The Rust Core Library
//!
//! The Rust Core Library is the dependency-free[^free] foundation of [The
//! Rust Standard Library](../std/index.html). It is the portable glue
//! between the language and its libraries, defining the intrinsic and
//! primitive building blocks of all Rust code. It links to no
//! upstream libraries, no system libraries, and no libc.
//!
//! [^free]: Strictly speaking, there are some symbols which are needed but
//!          they aren't always necessary.
//!
//! The core library is *minimal*: it isn't even aware of heap allocation,
//! nor does it provide concurrency or I/O. These things require
//! platform integration, and this library is platform-agnostic.
//!
//! # How to use the core library
//!
//! Please note that all of these details are currently not considered stable.
//!
// FIXME: Fill me in with more detail when the interface settles
//! This library is built on the assumption of a few existing symbols:
//!
//! * `memcpy`, `memcmp`, `memset` - These are core memory routines which are
//!   often generated by LLVM. Additionally, this library can make explicit
//!   calls to these functions. Their signatures are the same as found in C.
//!   These functions are often provided by the system libc, but can also be
//!   provided by the [compiler-builtins crate](https://crates.io/crates/compiler_builtins).
//!
//! * `rust_begin_panic` - This function takes four arguments, a
//!   `fmt::Arguments`, a `&'static str`, and two `u32`'s. These four arguments
//!   dictate the panic message, the file at which panic was invoked, and the
//!   line and column inside the file. It is up to consumers of this core
//!   library to define this panic function; it is only required to never
//!   return. This requires a `lang` attribute named `panic_impl`.
//!
//! * `rust_eh_personality` - is used by the failure mechanisms of the
//!    compiler. This is often mapped to GCC's personality function, but crates
//!    which do not trigger a panic can be assured that this function is never
//!    called. The `lang` attribute is called `eh_personality`.

// Since libcore defines many fundamental lang items, all tests live in a
// separate crate, libcoretest, to avoid bizarre issues.
//
// Here we explicitly #[cfg]-out this whole crate when testing. If we don't do
// this, both the generated test artifact and the linked libtest (which
// transitively includes libcore) will both define the same set of lang items,
// and this will cause the E0152 "found duplicate lang item" error. See
// discussion in #50466 for details.
//
// This cfg won't affect doc tests.
#![cfg(not(test))]
#![stable(feature = "core", since = "1.6.0")]
#![doc(
    html_root_url = "https://doc.rust-lang.org/nightly/",
    html_playground_url = "https://play.rust-lang.org/",
    issue_tracker_base_url = "https://github.com/rust-lang/rust/issues/",
    test(no_crate_inject, attr(deny(warnings))),
    test(attr(allow(dead_code, deprecated, unused_variables, unused_mut)))
)]
#![no_core]
#![warn(deprecated_in_future)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![allow(explicit_outlives_requirements)]
#![feature(rustc_allow_const_fn_unstable)]
#![feature(allow_internal_unstable)]
#![feature(arbitrary_self_types)]
#![feature(asm)]
#![feature(cfg_target_has_atomic)]
#![feature(const_heap)]
#![feature(const_alloc_layout)]
#![feature(const_assert_type)]
#![feature(const_discriminant)]
#![feature(const_cell_into_inner)]
#![feature(const_intrinsic_copy)]
#![feature(const_intrinsic_forget)]
#![feature(const_float_classify)]
#![feature(const_float_bits_conv)]
#![feature(const_int_unchecked_arith)]
#![feature(const_mut_refs)]
#![feature(const_refs_to_cell)]
#![feature(const_cttz)]
#![feature(const_panic)]
#![feature(const_pin)]
#![feature(const_fn)]
#![feature(const_fn_union)]
#![feature(const_impl_trait)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_option)]
#![feature(const_precise_live_drops)]
#![feature(const_ptr_offset)]
#![feature(const_ptr_offset_from)]
#![feature(const_ptr_read)]
#![feature(const_ptr_write)]
#![feature(const_raw_ptr_comparison)]
#![feature(const_raw_ptr_deref)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_slice_ptr_len)]
#![feature(const_size_of_val)]
#![feature(const_align_of_val)]
#![feature(const_type_id)]
#![feature(const_type_name)]
#![feature(const_likely)]
#![feature(const_unreachable_unchecked)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_maybe_uninit_as_ptr)]
#![feature(custom_inner_attributes)]
#![feature(decl_macro)]
#![feature(doc_cfg)]
#![feature(doc_spotlight)]
#![feature(duration_consts_2)]
#![feature(duration_saturating_ops)]
#![feature(extended_key_value_attributes)]
#![feature(extern_types)]
#![feature(fundamental)]
#![feature(intra_doc_pointers)]
#![feature(intrinsics)]
#![feature(lang_items)]
#![feature(link_llvm_intrinsics)]
#![feature(llvm_asm)]
#![feature(negative_impls)]
#![feature(never_type)]
#![feature(nll)]
#![feature(exhaustive_patterns)]
#![feature(no_core)]
#![feature(auto_traits)]
#![feature(or_patterns)]
#![feature(prelude_import)]
#![cfg_attr(not(bootstrap), feature(ptr_metadata))]
#![feature(repr_simd, platform_intrinsics)]
#![feature(rustc_attrs)]
#![feature(simd_ffi)]
#![feature(min_specialization)]
#![feature(staged_api)]
#![feature(std_internals)]
#![feature(stmt_expr_attributes)]
#![feature(str_split_as_str)]
#![feature(str_split_inclusive_as_str)]
#![feature(trait_alias)]
#![feature(transparent_unions)]
#![feature(try_blocks)]
#![feature(unboxed_closures)]
#![feature(unsized_fn_params)]
#![feature(unwind_attributes)]
#![feature(variant_count)]
#![feature(tbm_target_feature)]
#![feature(sse4a_target_feature)]
#![feature(arm_target_feature)]
#![feature(powerpc_target_feature)]
#![feature(mips_target_feature)]
#![feature(aarch64_target_feature)]
#![feature(wasm_target_feature)]
#![feature(avx512_target_feature)]
#![feature(cmpxchg16b_target_feature)]
#![feature(rtm_target_feature)]
#![feature(f16c_target_feature)]
#![feature(hexagon_target_feature)]
#![feature(const_fn_transmute)]
#![feature(abi_unadjusted)]
#![feature(adx_target_feature)]
#![feature(external_doc)]
#![feature(associated_type_bounds)]
#![feature(const_caller_location)]
#![feature(slice_ptr_get)]
#![feature(no_niche)] // rust-lang/rust#68303
#![feature(unsafe_block_in_unsafe_fn)]
#![feature(int_error_matching)]
#![deny(unsafe_op_in_unsafe_fn)]

#[prelude_import]
#[allow(unused)]
use prelude::v1::*;

#[cfg(not(test))] // See #65860
#[macro_use]
mod macros;

#[macro_use]
mod internal_macros;

#[path = "num/shells/int_macros.rs"]
#[macro_use]
mod int_macros;

#[path = "num/shells/i128.rs"]
pub mod i128;
#[path = "num/shells/i16.rs"]
pub mod i16;
#[path = "num/shells/i32.rs"]
pub mod i32;
#[path = "num/shells/i64.rs"]
pub mod i64;
#[path = "num/shells/i8.rs"]
pub mod i8;
#[path = "num/shells/isize.rs"]
pub mod isize;

#[path = "num/shells/u128.rs"]
pub mod u128;
#[path = "num/shells/u16.rs"]
pub mod u16;
#[path = "num/shells/u32.rs"]
pub mod u32;
#[path = "num/shells/u64.rs"]
pub mod u64;
#[path = "num/shells/u8.rs"]
pub mod u8;
#[path = "num/shells/usize.rs"]
pub mod usize;

#[path = "num/f32.rs"]
pub mod f32;
#[path = "num/f64.rs"]
pub mod f64;

#[macro_use]
pub mod num;

/* The libcore prelude, not as all-encompassing as the libstd prelude */

pub mod prelude;

/* Core modules for ownership management */

pub mod hint;
pub mod intrinsics;
pub mod mem;
pub mod ptr;

/* Core language traits */

pub mod borrow;
pub mod clone;
pub mod cmp;
pub mod convert;
pub mod default;
pub mod marker;
pub mod ops;

/* Core types and methods on primitives */

pub mod any;
pub mod array;
pub mod ascii;
pub mod cell;
pub mod char;
pub mod ffi;
pub mod iter;
#[unstable(feature = "once_cell", issue = "74465")]
pub mod lazy;
pub mod option;
pub mod panic;
pub mod panicking;
pub mod pin;
pub mod raw;
pub mod result;
#[unstable(feature = "async_stream", issue = "79024")]
pub mod stream;
pub mod sync;

pub mod fmt;
pub mod hash;
pub mod slice;
pub mod str;
pub mod time;

pub mod unicode;

/* Async */
pub mod future;
pub mod task;

/* Heap memory allocator trait */
#[allow(missing_docs)]
pub mod alloc;

// note: does not need to be public
mod bool;
mod tuple;
mod unit;

#[stable(feature = "core_primitive", since = "1.43.0")]
pub mod primitive;

// Pull in the `core_arch` crate directly into libcore. The contents of
// `core_arch` are in a different repository: rust-lang/stdarch.
//
// `core_arch` depends on libcore, but the contents of this module are
// set up in such a way that directly pulling it here works such that the
// crate uses the this crate as its libcore.
#[path = "../../stdarch/crates/core_arch/src/mod.rs"]
#[allow(
    missing_docs,
    missing_debug_implementations,
    dead_code,
    unused_imports,
    unsafe_op_in_unsafe_fn
)]
#[allow(non_autolinks)]
// FIXME: This annotation should be moved into rust-lang/stdarch after clashing_extern_declarations is
// merged. It currently cannot because bootstrap fails as the lint hasn't been defined yet.
#[allow(clashing_extern_declarations)]
#[unstable(feature = "stdsimd", issue = "48556")]
mod core_arch;

#[stable(feature = "simd_arch", since = "1.27.0")]
pub use core_arch::arch;
