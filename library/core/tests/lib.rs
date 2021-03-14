#![feature(alloc_layout_extra)]
#![feature(array_chunks)]
#![feature(array_from_ref)]
#![feature(array_methods)]
#![feature(array_map)]
#![feature(array_windows)]
#![feature(bool_to_option)]
#![feature(bound_cloned)]
#![feature(box_syntax)]
#![feature(cell_update)]
#![feature(cfg_panic)]
#![feature(cfg_target_has_atomic)]
#![feature(const_assume)]
#![feature(const_cell_into_inner)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_ptr_read)]
#![feature(const_ptr_write)]
#![feature(const_ptr_offset)]
#![feature(control_flow_enum)]
#![feature(core_intrinsics)]
#![feature(core_private_bignum)]
#![feature(core_private_diy_float)]
#![feature(debug_non_exhaustive)]
#![feature(dec2flt)]
#![feature(div_duration)]
#![feature(duration_consts_2)]
#![feature(duration_constants)]
#![feature(duration_saturating_ops)]
#![feature(duration_zero)]
#![feature(exact_size_is_empty)]
#![feature(extern_types)]
#![feature(fixed_size_array)]
#![feature(flt2dec)]
#![feature(fmt_internals)]
#![feature(hashmap_internals)]
#![feature(try_find)]
#![feature(is_sorted)]
#![feature(pattern)]
#![feature(raw)]
#![feature(sort_internals)]
#![feature(slice_partition_at_index)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_write_slice)]
#![feature(min_specialization)]
#![feature(step_trait)]
#![feature(step_trait_ext)]
#![feature(str_internals)]
#![feature(test)]
#![feature(trusted_len)]
#![feature(try_trait)]
#![feature(slice_internals)]
#![feature(slice_partition_dedup)]
#![feature(int_error_matching)]
#![feature(iter_advance_by)]
#![feature(iter_partition_in_place)]
#![feature(iter_intersperse)]
#![feature(iter_is_partitioned)]
#![feature(iter_order_by)]
#![feature(cmp_min_max_by)]
#![feature(iter_map_while)]
#![feature(const_mut_refs)]
#![feature(const_pin)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_raw_ptr_deref)]
#![feature(never_type)]
#![feature(unwrap_infallible)]
#![feature(option_result_unwrap_unchecked)]
#![feature(result_into_ok_or_err)]
#![feature(peekable_peek_mut)]
#![cfg_attr(not(bootstrap), feature(ptr_metadata))]
#![feature(once_cell)]
#![feature(unsized_tuple_coercion)]
#![feature(int_bits_const)]
#![feature(nonzero_leading_trailing_zeros)]
#![feature(const_option)]
#![feature(integer_atomics)]
#![feature(slice_group_by)]
#![feature(trusted_random_access)]
#![cfg_attr(bootstrap, feature(unsafe_block_in_unsafe_fn))]
#![cfg_attr(not(bootstrap), feature(unsize))]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate test;

mod alloc;
mod any;
mod array;
mod ascii;
mod atomic;
mod bool;
mod cell;
mod char;
mod clone;
mod cmp;
mod const_ptr;
mod fmt;
mod hash;
mod intrinsics;
mod iter;
mod lazy;
mod macros;
mod manually_drop;
mod mem;
mod nonzero;
mod num;
mod ops;
mod option;
mod pattern;
mod pin;
mod ptr;
mod result;
mod slice;
mod str;
mod str_lossy;
mod task;
mod time;
mod tuple;
mod unicode;
