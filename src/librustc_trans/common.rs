// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types, non_snake_case)]

//! Code that is useful in various trans modules.

use llvm;
use llvm::{ValueRef, ContextRef, TypeKind};
use llvm::{True, False, Bool, OperandBundleDef};
use rustc::hir::def_id::DefId;
use rustc::middle::lang_items::LangItem;
use abi;
use base;
use builder::Builder;
use consts;
use declare;
use type_::Type;
use type_of::LayoutLlvmExt;
use value::Value;
use rustc::traits;
use rustc::ty::{self, Ty, TyCtxt};
use rustc::ty::layout::{HasDataLayout, LayoutOf};
use rustc::ty::subst::Kind;
use rustc::hir;

use libc::{c_uint, c_char};
use std::iter;

use syntax::abi::Abi;
use syntax::symbol::InternedString;
use syntax_pos::{Span, DUMMY_SP};

pub use context::CodegenCx;

pub fn type_needs_drop<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, ty: Ty<'tcx>) -> bool {
    ty.needs_drop(tcx, ty::ParamEnv::empty(traits::Reveal::All))
}

pub fn type_is_sized<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_sized(tcx, ty::ParamEnv::empty(traits::Reveal::All), DUMMY_SP)
}

pub fn type_is_freeze<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_freeze(tcx, ty::ParamEnv::empty(traits::Reveal::All), DUMMY_SP)
}

/*
* A note on nomenclature of linking: "extern", "foreign", and "upcall".
*
* An "extern" is an LLVM symbol we wind up emitting an undefined external
* reference to. This means "we don't have the thing in this compilation unit,
* please make sure you link it in at runtime". This could be a reference to
* C code found in a C library, or rust code found in a rust crate.
*
* Most "externs" are implicitly declared (automatically) as a result of a
* user declaring an extern _module_ dependency; this causes the rust driver
* to locate an extern crate, scan its compilation metadata, and emit extern
* declarations for any symbols used by the declaring crate.
*
* A "foreign" is an extern that references C (or other non-rust ABI) code.
* There is no metadata to scan for extern references so in these cases either
* a header-digester like bindgen, or manual function prototypes, have to
* serve as declarators. So these are usually given explicitly as prototype
* declarations, in rust code, with ABI attributes on them noting which ABI to
* link via.
*
* An "upcall" is a foreign call generated by the compiler (not corresponding
* to any user-written call in the code) into the runtime library, to perform
* some helper task such as bringing a task to life, allocating memory, etc.
*
*/

/// A structure representing an active landing pad for the duration of a basic
/// block.
///
/// Each `Block` may contain an instance of this, indicating whether the block
/// is part of a landing pad or not. This is used to make decision about whether
/// to emit `invoke` instructions (e.g. in a landing pad we don't continue to
/// use `invoke`) and also about various function call metadata.
///
/// For GNU exceptions (`landingpad` + `resume` instructions) this structure is
/// just a bunch of `None` instances (not too interesting), but for MSVC
/// exceptions (`cleanuppad` + `cleanupret` instructions) this contains data.
/// When inside of a landing pad, each function call in LLVM IR needs to be
/// annotated with which landing pad it's a part of. This is accomplished via
/// the `OperandBundleDef` value created for MSVC landing pads.
pub struct Funclet {
    cleanuppad: ValueRef,
    operand: OperandBundleDef,
}

impl Funclet {
    pub fn new(cleanuppad: ValueRef) -> Funclet {
        Funclet {
            cleanuppad,
            operand: OperandBundleDef::new("funclet", &[cleanuppad]),
        }
    }

    pub fn cleanuppad(&self) -> ValueRef {
        self.cleanuppad
    }

    pub fn bundle(&self) -> &OperandBundleDef {
        &self.operand
    }
}

pub fn val_ty(v: ValueRef) -> Type {
    unsafe {
        Type::from_ref(llvm::LLVMTypeOf(v))
    }
}

// LLVM constant constructors.
pub fn C_null(t: Type) -> ValueRef {
    unsafe {
        llvm::LLVMConstNull(t.to_ref())
    }
}

pub fn C_undef(t: Type) -> ValueRef {
    unsafe {
        llvm::LLVMGetUndef(t.to_ref())
    }
}

pub fn C_int(t: Type, i: i64) -> ValueRef {
    unsafe {
        llvm::LLVMConstInt(t.to_ref(), i as u64, True)
    }
}

pub fn C_uint(t: Type, i: u64) -> ValueRef {
    unsafe {
        llvm::LLVMConstInt(t.to_ref(), i, False)
    }
}

pub fn C_uint_big(t: Type, u: u128) -> ValueRef {
    unsafe {
        let words = [u as u64, (u >> 64) as u64];
        llvm::LLVMConstIntOfArbitraryPrecision(t.to_ref(), 2, words.as_ptr())
    }
}

pub fn C_bool(ccx: &CodegenCx, val: bool) -> ValueRef {
    C_uint(Type::i1(ccx), val as u64)
}

pub fn C_i32(ccx: &CodegenCx, i: i32) -> ValueRef {
    C_int(Type::i32(ccx), i as i64)
}

pub fn C_u32(ccx: &CodegenCx, i: u32) -> ValueRef {
    C_uint(Type::i32(ccx), i as u64)
}

pub fn C_u64(ccx: &CodegenCx, i: u64) -> ValueRef {
    C_uint(Type::i64(ccx), i)
}

pub fn C_usize(ccx: &CodegenCx, i: u64) -> ValueRef {
    let bit_size = ccx.data_layout().pointer_size.bits();
    if bit_size < 64 {
        // make sure it doesn't overflow
        assert!(i < (1<<bit_size));
    }

    C_uint(ccx.isize_ty, i)
}

pub fn C_u8(ccx: &CodegenCx, i: u8) -> ValueRef {
    C_uint(Type::i8(ccx), i as u64)
}


// This is a 'c-like' raw string, which differs from
// our boxed-and-length-annotated strings.
pub fn C_cstr(cx: &CodegenCx, s: InternedString, null_terminated: bool) -> ValueRef {
    unsafe {
        if let Some(&llval) = cx.const_cstr_cache.borrow().get(&s) {
            return llval;
        }

        let sc = llvm::LLVMConstStringInContext(cx.llcx,
                                                s.as_ptr() as *const c_char,
                                                s.len() as c_uint,
                                                !null_terminated as Bool);
        let sym = cx.generate_local_symbol_name("str");
        let g = declare::define_global(cx, &sym[..], val_ty(sc)).unwrap_or_else(||{
            bug!("symbol `{}` is already defined", sym);
        });
        llvm::LLVMSetInitializer(g, sc);
        llvm::LLVMSetGlobalConstant(g, True);
        llvm::LLVMRustSetLinkage(g, llvm::Linkage::InternalLinkage);

        cx.const_cstr_cache.borrow_mut().insert(s, g);
        g
    }
}

// NB: Do not use `do_spill_noroot` to make this into a constant string, or
// you will be kicked off fast isel. See issue #4352 for an example of this.
pub fn C_str_slice(cx: &CodegenCx, s: InternedString) -> ValueRef {
    let len = s.len();
    let cs = consts::ptrcast(C_cstr(cx, s, false),
        cx.layout_of(cx.tcx.mk_str()).llvm_type(cx).ptr_to());
    C_fat_ptr(cx, cs, C_usize(cx, len as u64))
}

pub fn C_fat_ptr(cx: &CodegenCx, ptr: ValueRef, meta: ValueRef) -> ValueRef {
    assert_eq!(abi::FAT_PTR_ADDR, 0);
    assert_eq!(abi::FAT_PTR_EXTRA, 1);
    C_struct(cx, &[ptr, meta], false)
}

pub fn C_struct(cx: &CodegenCx, elts: &[ValueRef], packed: bool) -> ValueRef {
    C_struct_in_context(cx.llcx, elts, packed)
}

pub fn C_struct_in_context(llcx: ContextRef, elts: &[ValueRef], packed: bool) -> ValueRef {
    unsafe {
        llvm::LLVMConstStructInContext(llcx,
                                       elts.as_ptr(), elts.len() as c_uint,
                                       packed as Bool)
    }
}

pub fn C_array(ty: Type, elts: &[ValueRef]) -> ValueRef {
    unsafe {
        return llvm::LLVMConstArray(ty.to_ref(), elts.as_ptr(), elts.len() as c_uint);
    }
}

pub fn C_vector(elts: &[ValueRef]) -> ValueRef {
    unsafe {
        return llvm::LLVMConstVector(elts.as_ptr(), elts.len() as c_uint);
    }
}

pub fn C_bytes(cx: &CodegenCx, bytes: &[u8]) -> ValueRef {
    C_bytes_in_context(cx.llcx, bytes)
}

pub fn C_bytes_in_context(llcx: ContextRef, bytes: &[u8]) -> ValueRef {
    unsafe {
        let ptr = bytes.as_ptr() as *const c_char;
        return llvm::LLVMConstStringInContext(llcx, ptr, bytes.len() as c_uint, True);
    }
}

pub fn const_get_elt(v: ValueRef, idx: u64) -> ValueRef {
    unsafe {
        assert_eq!(idx as c_uint as u64, idx);
        let us = &[idx as c_uint];
        let r = llvm::LLVMConstExtractValue(v, us.as_ptr(), us.len() as c_uint);

        debug!("const_get_elt(v={:?}, idx={}, r={:?})",
               Value(v), idx, Value(r));

        r
    }
}

pub fn const_to_uint(v: ValueRef) -> u64 {
    unsafe {
        llvm::LLVMConstIntGetZExtValue(v)
    }
}

pub fn is_const_integral(v: ValueRef) -> bool {
    unsafe {
        !llvm::LLVMIsAConstantInt(v).is_null()
    }
}

#[inline]
fn hi_lo_to_u128(lo: u64, hi: u64) -> u128 {
    ((hi as u128) << 64) | (lo as u128)
}

pub fn const_to_opt_u128(v: ValueRef, sign_ext: bool) -> Option<u128> {
    unsafe {
        if is_const_integral(v) {
            let (mut lo, mut hi) = (0u64, 0u64);
            let success = llvm::LLVMRustConstInt128Get(v, sign_ext,
                                                       &mut hi as *mut u64, &mut lo as *mut u64);
            if success {
                Some(hi_lo_to_u128(lo, hi))
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub fn langcall(tcx: TyCtxt,
                span: Option<Span>,
                msg: &str,
                li: LangItem)
                -> DefId {
    match tcx.lang_items().require(li) {
        Ok(id) => id,
        Err(s) => {
            let msg = format!("{} {}", msg, s);
            match span {
                Some(span) => tcx.sess.span_fatal(span, &msg[..]),
                None => tcx.sess.fatal(&msg[..]),
            }
        }
    }
}

// To avoid UB from LLVM, these two functions mask RHS with an
// appropriate mask unconditionally (i.e. the fallback behavior for
// all shifts). For 32- and 64-bit types, this matches the semantics
// of Java. (See related discussion on #1877 and #10183.)

pub fn build_unchecked_lshift<'a, 'tcx>(
    bcx: &Builder<'a, 'tcx>,
    lhs: ValueRef,
    rhs: ValueRef
) -> ValueRef {
    let rhs = base::cast_shift_expr_rhs(bcx, hir::BinOp_::BiShl, lhs, rhs);
    // #1877, #10183: Ensure that input is always valid
    let rhs = shift_mask_rhs(bcx, rhs);
    bcx.shl(lhs, rhs)
}

pub fn build_unchecked_rshift<'a, 'tcx>(
    bcx: &Builder<'a, 'tcx>, lhs_t: Ty<'tcx>, lhs: ValueRef, rhs: ValueRef
) -> ValueRef {
    let rhs = base::cast_shift_expr_rhs(bcx, hir::BinOp_::BiShr, lhs, rhs);
    // #1877, #10183: Ensure that input is always valid
    let rhs = shift_mask_rhs(bcx, rhs);
    let is_signed = lhs_t.is_signed();
    if is_signed {
        bcx.ashr(lhs, rhs)
    } else {
        bcx.lshr(lhs, rhs)
    }
}

fn shift_mask_rhs<'a, 'tcx>(bcx: &Builder<'a, 'tcx>, rhs: ValueRef) -> ValueRef {
    let rhs_llty = val_ty(rhs);
    bcx.and(rhs, shift_mask_val(bcx, rhs_llty, rhs_llty, false))
}

pub fn shift_mask_val<'a, 'tcx>(
    bcx: &Builder<'a, 'tcx>,
    llty: Type,
    mask_llty: Type,
    invert: bool
) -> ValueRef {
    let kind = llty.kind();
    match kind {
        TypeKind::Integer => {
            // i8/u8 can shift by at most 7, i16/u16 by at most 15, etc.
            let val = llty.int_width() - 1;
            if invert {
                C_int(mask_llty, !val as i64)
            } else {
                C_uint(mask_llty, val)
            }
        },
        TypeKind::Vector => {
            let mask = shift_mask_val(bcx, llty.element_type(), mask_llty.element_type(), invert);
            bcx.vector_splat(mask_llty.vector_length(), mask)
        },
        _ => bug!("shift_mask_val: expected Integer or Vector, found {:?}", kind),
    }
}

pub fn ty_fn_sig<'a, 'tcx>(ccx: &CodegenCx<'a, 'tcx>,
                           ty: Ty<'tcx>)
                           -> ty::PolyFnSig<'tcx>
{
    match ty.sty {
        ty::TyFnDef(..) |
        // Shims currently have type TyFnPtr. Not sure this should remain.
        ty::TyFnPtr(_) => ty.fn_sig(ccx.tcx),
        ty::TyClosure(def_id, substs) => {
            let tcx = ccx.tcx;
            let sig = substs.closure_sig(def_id, tcx);

            let env_ty = tcx.closure_env_ty(def_id, substs).unwrap();
            sig.map_bound(|sig| tcx.mk_fn_sig(
                iter::once(*env_ty.skip_binder()).chain(sig.inputs().iter().cloned()),
                sig.output(),
                sig.variadic,
                sig.unsafety,
                sig.abi
            ))
        }
        ty::TyGenerator(def_id, substs, _) => {
            let tcx = ccx.tcx;
            let sig = substs.generator_poly_sig(def_id, ccx.tcx);

            let env_region = ty::ReLateBound(ty::DebruijnIndex::new(1), ty::BrEnv);
            let env_ty = tcx.mk_mut_ref(tcx.mk_region(env_region), ty);

            sig.map_bound(|sig| {
                let state_did = tcx.lang_items().gen_state().unwrap();
                let state_adt_ref = tcx.adt_def(state_did);
                let state_substs = tcx.mk_substs([Kind::from(sig.yield_ty),
                    Kind::from(sig.return_ty)].iter());
                let ret_ty = tcx.mk_adt(state_adt_ref, state_substs);

                tcx.mk_fn_sig(iter::once(env_ty),
                    ret_ty,
                    false,
                    hir::Unsafety::Normal,
                    Abi::Rust
                )
            })
        }
        _ => bug!("unexpected type {:?} to ty_fn_sig", ty)
    }
}

