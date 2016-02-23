// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Handles translation of callees as well as other call-related
//! things.  Callees are a superset of normal rust values and sometimes
//! have different representations.  In particular, top-level fn items
//! and methods are represented as just a fn ptr and not a full
//! closure.

pub use self::CalleeData::*;
pub use self::CallArgs::*;

use arena::TypedArena;
use back::link;
use llvm::{self, ValueRef, get_params};
use middle::cstore::LOCAL_CRATE;
use middle::def_id::DefId;
use middle::infer;
use middle::subst;
use middle::subst::{Substs};
use rustc::front::map as hir_map;
use trans::adt;
use trans::base;
use trans::base::*;
use trans::build::*;
use trans::cleanup;
use trans::cleanup::CleanupMethods;
use trans::common::{self, Block, Result, NodeIdAndSpan, ExprId, CrateContext,
                    ExprOrMethodCall, FunctionContext, MethodCallKey};
use trans::consts;
use trans::datum::*;
use trans::debuginfo::DebugLoc;
use trans::declare;
use trans::expr;
use trans::glue;
use trans::inline;
use trans::foreign;
use trans::intrinsic;
use trans::meth;
use trans::monomorphize;
use trans::type_::Type;
use trans::type_of;
use trans::value::Value;
use trans::Disr;
use middle::ty::{self, Ty, TyCtxt, TypeFoldable};
use rustc_front::hir;

use syntax::abi::Abi;
use syntax::ast;
use syntax::codemap::DUMMY_SP;
use syntax::errors;
use syntax::ptr::P;

pub enum CalleeData<'tcx> {
    /// Constructor for enum variant/tuple-like-struct.
    NamedTupleConstructor(Disr),

    /// Function pointer.
    Fn(ValueRef),

    Intrinsic(ast::NodeId, &'tcx subst::Substs<'tcx>),

    /// Trait object found in the vtable at that index.
    Virtual(usize)
}

pub struct Callee<'tcx> {
    pub data: CalleeData<'tcx>,
    pub ty: Ty<'tcx>
}

impl<'tcx> Callee<'tcx> {
    /// Function pointer.
    pub fn ptr(datum: Datum<'tcx, Rvalue>) -> Callee<'tcx> {
        Callee {
            data: Fn(datum.val),
            ty: datum.ty
        }
    }

    /// Trait or impl method call.
    pub fn method_call<'blk>(bcx: Block<'blk, 'tcx>,
                             method_call: ty::MethodCall)
                             -> Callee<'tcx> {
        let method = bcx.tcx().tables.borrow().method_map[&method_call];
        Callee::method(bcx, method)
    }

    /// Trait or impl method.
    pub fn method<'blk>(bcx: Block<'blk, 'tcx>,
                        method: ty::MethodCallee<'tcx>) -> Callee<'tcx> {
        let substs = bcx.tcx().mk_substs(bcx.fcx.monomorphize(&method.substs));
        Callee::def(bcx.ccx(), method.def_id, substs)
    }

    /// Function or method definition.
    pub fn def<'a>(ccx: &CrateContext<'a, 'tcx>,
                   def_id: DefId,
                   substs: &'tcx subst::Substs<'tcx>)
                   -> Callee<'tcx> {
        let tcx = ccx.tcx();

        if substs.self_ty().is_some() {
            // Only trait methods can have a Self parameter.
            return Callee::trait_method(ccx, def_id, substs);
        }

        let maybe_node_id = inline::get_local_instance(ccx, def_id)
            .and_then(|def_id| tcx.map.as_local_node_id(def_id));
        let maybe_ast_node = maybe_node_id.and_then(|node_id| {
            tcx.map.find(node_id)
        });

        let data = match maybe_ast_node {
            Some(hir_map::NodeStructCtor(_)) => {
                NamedTupleConstructor(Disr(0))
            }
            Some(hir_map::NodeVariant(_)) => {
                let vinfo = common::inlined_variant_def(ccx, maybe_node_id.unwrap());
                NamedTupleConstructor(Disr::from(vinfo.disr_val))
            }
            Some(hir_map::NodeForeignItem(fi)) if {
                let abi = tcx.map.get_foreign_abi(fi.id);
                abi == Abi::RustIntrinsic || abi == Abi::PlatformIntrinsic
            } => Intrinsic,

            _ => return Callee::ptr(get_fn(ccx, def_id, substs))
        };

        Callee {
            data: data,
            ty: def_ty(tcx, def_id, substs)
        }
    }

    /// Trait method, which has to be resolved to an impl method.
    pub fn trait_method<'a>(ccx: &CrateContext<'a, 'tcx>,
                            def_id: DefId,
                            substs: &'tcx subst::Substs<'tcx>)
                            -> Callee<'tcx> {
        let tcx = ccx.tcx();

        let method_item = tcx.impl_or_trait_item(def_id);
        let trait_id = method_item.container().id();
        let trait_ref = ty::Binder(substs.to_trait_ref(tcx, trait_id));
        match common::fulfill_obligation(ccx, DUMMY_SP, trait_ref) {
            traits::VtableImpl(vtable_impl) => {
                let impl_did = vtable_impl.impl_def_id;
                let mname = tcx.item_name(def_id);
                // create a concatenated set of substitutions which includes
                // those from the impl and those from the method:
                let impl_substs = vtable_impl.substs.with_method_from(&substs);
                let substs = tcx.mk_substs(impl_substs);
                let mth = meth::get_impl_method(tcx, impl_did, substs, mname);

                // Translate the function, bypassing Callee::def.
                // That is because default methods have the same ID as the
                // trait method used to look up the impl method that ended
                // up here, so calling Callee::def would infinitely recurse.
                Callee::ptr(get_fn(ccx, mth.method.def_id, mth.substs))
            }
            traits::VtableClosure(vtable_closure) => {
                // The substitutions should have no type parameters remaining
                // after passing through fulfill_obligation
                let trait_closure_kind = tcx.lang_items.fn_trait_kind(trait_id).unwrap();
                let llfn = closure::trans_closure_method(ccx,
                                                         vtable_closure.closure_def_id,
                                                         vtable_closure.substs,
                                                         trait_closure_kind);

                let method_ty = def_ty(tcx, def_id, substs);
                let fn_ptr_ty = match method_ty.sty {
                    ty::TyFnDef(_, _, fty) => tcx.mk_ty(ty::TyFnPtr(fty)),
                    _ => unreachable!("expected fn item type, found {}",
                                      method_ty)
                };
                Callee::ptr(immediate_rvalue(llfn, fn_ptr_ty))
            }
            traits::VtableFnPointer(fn_ty) => {
                let trait_closure_kind = tcx.lang_items.fn_trait_kind(trait_id).unwrap();
                let llfn = trans_fn_pointer_shim(ccx, trait_closure_kind, fn_ty);

                let method_ty = def_ty(tcx, def_id, substs);
                let fn_ptr_ty = match method_ty.sty {
                    ty::TyFnDef(_, _, fty) => tcx.mk_ty(ty::TyFnPtr(fty)),
                    _ => unreachable!("expected fn item type, found {}",
                                      method_ty)
                };
                Callee::ptr(immediate_rvalue(llfn, fn_ptr_ty))
            }
            traits::VtableObject(ref data) => {
                Callee {
                    data: Virtual(traits::get_vtable_index_of_object_method(
                        tcx, data, def_id)),
                    ty: def_ty(tcx, def_id, substs)
                }
            }
            vtable => {
                unreachable!("resolved vtable bad vtable {:?} in trans", vtable);
            }
        }
    }

    /// This behemoth of a function translates function calls. Unfortunately, in
    /// order to generate more efficient LLVM output at -O0, it has quite a complex
    /// signature (refactoring this into two functions seems like a good idea).
    ///
    /// In particular, for lang items, it is invoked with a dest of None, and in
    /// that case the return value contains the result of the fn. The lang item must
    /// not return a structural type or else all heck breaks loose.
    ///
    /// For non-lang items, `dest` is always Some, and hence the result is written
    /// into memory somewhere. Nonetheless we return the actual return value of the
    /// function.
    pub fn call<'a, 'blk>(self, bcx: Block<'blk, 'tcx>,
                          debug_loc: DebugLoc,
                          args: CallArgs<'a, 'tcx>,
                          dest: Option<expr::Dest>)
                          -> Result<'blk, 'tcx> {
        trans_call_inner(bcx, debug_loc, self, args, dest)
    }

    /// Turn the callee into a function pointer.
    pub fn reify<'a>(self, ccx: &CrateContext<'a, 'tcx>)
                     -> Datum<'tcx, Rvalue> {
        match self.data {
            Fn(llfn) => {
                let fn_ptr_ty = match self.ty.sty {
                    ty::TyFnDef(_, _, f) => ccx.tcx().mk_ty(ty::TyFnPtr(f)),
                    _ => self.ty
                };
                immediate_rvalue(llfn, fn_ptr_ty)
            }
            Virtual(idx) => meth::trans_object_shim(ccx, self.ty, idx),
            NamedTupleConstructor(_) => match self.ty.sty {
                ty::TyFnDef(def_id, substs, _) => {
                    return get_fn(ccx, def_id, substs);
                }
                _ => unreachable!("expected fn item type, found {}", self.ty)
            },
            Intrinsic(..) => unreachable!("intrinsic {} getting reified", self.ty)
        }
    }
}

/// Given a DefId and some Substs, produces the monomorphic item type.
fn def_ty<'tcx>(tcx: &TyCtxt<'tcx>,
                def_id: DefId,
                substs: &'tcx subst::Substs<'tcx>)
                -> Ty<'tcx> {
    let ty = tcx.lookup_item_type(def_id).ty;
    monomorphize::apply_param_substs(tcx, substs, &ty)
}

/// Translates an adapter that implements the `Fn` trait for a fn
/// pointer. This is basically the equivalent of something like:
///
/// ```
/// impl<'a> Fn(&'a int) -> &'a int for fn(&int) -> &int {
///     extern "rust-abi" fn call(&self, args: (&'a int,)) -> &'a int {
///         (*self)(args.0)
///     }
/// }
/// ```
///
/// but for the bare function type given.
pub fn trans_fn_pointer_shim<'a, 'tcx>(
    ccx: &'a CrateContext<'a, 'tcx>,
    closure_kind: ty::ClosureKind,
    bare_fn_ty: Ty<'tcx>)
    -> ValueRef
{
    let _icx = push_ctxt("trans_fn_pointer_shim");
    let tcx = ccx.tcx();

    // Normalize the type for better caching.
    let bare_fn_ty = tcx.erase_regions(&bare_fn_ty);

    // If this is an impl of `Fn` or `FnMut` trait, the receiver is `&self`.
    let is_by_ref = match closure_kind {
        ty::ClosureKind::Fn | ty::ClosureKind::FnMut => true,
        ty::ClosureKind::FnOnce => false,
    };
    let bare_fn_ty_maybe_ref = if is_by_ref {
        tcx.mk_imm_ref(tcx.mk_region(ty::ReStatic), bare_fn_ty)
    } else {
        bare_fn_ty
    };

    // Check if we already trans'd this shim.
    match ccx.fn_pointer_shims().borrow().get(&bare_fn_ty_maybe_ref) {
        Some(&llval) => { return llval; }
        None => { }
    }

    debug!("trans_fn_pointer_shim(bare_fn_ty={:?})",
           bare_fn_ty);

    // Construct the "tuply" version of `bare_fn_ty`. It takes two arguments: `self`,
    // which is the fn pointer, and `args`, which is the arguments tuple.
    let sig = match bare_fn_ty.sty {
        ty::TyFnDef(_, _,
                    &ty::BareFnTy { unsafety: hir::Unsafety::Normal,
                                    abi: Abi::Rust,
                                    ref sig }) |
        ty::TyFnPtr(&ty::BareFnTy { unsafety: hir::Unsafety::Normal,
                                    abi: Abi::Rust,
                                    ref sig }) => sig,

        _ => {
            tcx.sess.bug(&format!("trans_fn_pointer_shim invoked on invalid type: {}",
                                    bare_fn_ty));
        }
    };
    let sig = tcx.erase_late_bound_regions(sig);
    let sig = infer::normalize_associated_type(ccx.tcx(), &sig);
    let tuple_input_ty = tcx.mk_tup(sig.inputs.to_vec());
    let tuple_fn_ty = tcx.mk_fn_ptr(ty::BareFnTy {
        unsafety: hir::Unsafety::Normal,
        abi: Abi::RustCall,
        sig: ty::Binder(ty::FnSig {
            inputs: vec![bare_fn_ty_maybe_ref,
                         tuple_input_ty],
            output: sig.output,
            variadic: false
        })
    });
    debug!("tuple_fn_ty: {:?}", tuple_fn_ty);

    //
    let function_name = link::mangle_internal_name_by_type_and_seq(ccx, bare_fn_ty,
                                                                   "fn_pointer_shim");
    let llfn = declare::declare_internal_rust_fn(ccx, &function_name[..], tuple_fn_ty);

    //
    let empty_substs = tcx.mk_substs(Substs::trans_empty());
    let (block_arena, fcx): (TypedArena<_>, FunctionContext);
    block_arena = TypedArena::new();
    fcx = new_fn_ctxt(ccx,
                      llfn,
                      ast::DUMMY_NODE_ID,
                      false,
                      sig.output,
                      empty_substs,
                      None,
                      &block_arena);
    let mut bcx = init_function(&fcx, false, sig.output);

    let llargs = get_params(fcx.llfn);

    let self_idx = fcx.arg_offset();
    let llfnpointer = match bare_fn_ty.sty {
        ty::TyFnDef(def_id, substs, _) => {
            // Function definitions have to be turned into a pointer.
            Callee::def(ccx, def_id, substs).reify(ccx).val
        }

        // the first argument (`self`) will be ptr to the fn pointer
        _ => if is_by_ref {
            Load(bcx, llargs[self_idx])
        } else {
            llargs[self_idx]
        }
    };

    assert!(!fcx.needs_ret_allocas);

    let dest = fcx.llretslotptr.get().map(|_|
        expr::SaveIn(fcx.get_ret_slot(bcx, sig.output, "ret_slot"))
    );

    let callee = Callee {
        data: Fn(llfnpointer),
        ty: bare_fn_ty
    };
    bcx = callee.call(bcx, DebugLoc::None, ArgVals(&llargs[(self_idx + 1)..]), dest).bcx;

    finish_fn(&fcx, bcx, sig.output, DebugLoc::None);

    ccx.fn_pointer_shims().borrow_mut().insert(bare_fn_ty_maybe_ref, llfn);

    llfn
}

/// Translates a reference to a fn/method item, monomorphizing and
/// inlining as it goes.
///
/// # Parameters
///
/// - `ccx`: the crate context
/// - `def_id`: def id of the fn or method item being referenced
/// - `substs`: values for each of the fn/method's parameters
fn get_fn<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                    def_id: DefId,
                    substs: &'tcx subst::Substs<'tcx>)
                    -> Datum<'tcx, Rvalue> {
    let tcx = ccx.tcx();

    debug!("get_fn(def_id={:?}, substs={:?})", def_id, substs);

    assert!(!substs.types.needs_infer());
    assert!(!substs.types.has_escaping_regions());

    // Check whether this fn has an inlined copy and, if so, redirect
    // def_id to the local id of the inlined copy.
    let def_id = inline::maybe_instantiate_inline(ccx, def_id);

    fn is_named_tuple_constructor(tcx: &TyCtxt, def_id: DefId) -> bool {
        let node_id = match tcx.map.as_local_node_id(def_id) {
            Some(n) => n,
            None => { return false; }
        };
        let map_node = errors::expect(
            &tcx.sess.diagnostic(),
            tcx.map.find(node_id),
            || "local item should be in ast map".to_string());

        match map_node {
            hir_map::NodeVariant(v) => {
                v.node.data.is_tuple()
            }
            hir_map::NodeStructCtor(_) => true,
            _ => false
        }
    }
    let must_monomorphise =
        !substs.types.is_empty() || is_named_tuple_constructor(tcx, def_id);

    debug!("get_fn({:?}) must_monomorphise: {}",
           def_id, must_monomorphise);

    // Create a monomorphic version of generic functions
    if must_monomorphise {
        // Should be either intra-crate or inlined.
        assert_eq!(def_id.krate, LOCAL_CRATE);

        let substs = tcx.mk_substs(substs.clone().erase_regions());
        let (mut val, fn_ty, must_cast) =
            monomorphize::monomorphic_fn(ccx, def_id, substs);
        let fn_ty = ref_ty.unwrap_or(fn_ty);
        let fn_ptr_ty = match fn_ty.sty {
            ty::TyFnDef(_, _, fty) => {
                // Create a fn pointer with the substituted signature.
                tcx.mk_ty(ty::TyFnPtr(fty))
            }
            _ => unreachable!("expected fn item type, found {}", fn_ty)
        };
        if must_cast && ref_ty.is_some() {
            let llptrty = type_of::type_of(ccx, fn_ptr_ty);
            if llptrty != common::val_ty(val) {
                val = consts::ptrcast(val, llptrty);
            }
        }
        return immediate_rvalue(val, fn_ptr_ty);
    }

    // Find the actual function pointer.
    let local_node = ccx.tcx().map.as_local_node_id(def_id);
    let mut datum = if let Some(node_id) = local_node {
        // Type scheme of the function item (may have type params)
        let fn_type_scheme = tcx.lookup_item_type(def_id);
        let fn_type = match fn_type_scheme.ty.sty {
            ty::TyFnDef(_, _, fty) => {
                // Create a fn pointer with the normalized signature.
                tcx.mk_fn_ptr(infer::normalize_associated_type(tcx, fty))
            }
            _ => unreachable!("expected fn item type, found {}",
                              fn_type_scheme.ty)
        };

        // Internal reference.
        immediate_rvalue(get_item_val(ccx, node_id), fn_type)
    } else {
        // External reference.
        get_extern_fn(ccx, def_id)
    };

    // This is subtle and surprising, but sometimes we have to bitcast
    // the resulting fn pointer.  The reason has to do with external
    // functions.  If you have two crates that both bind the same C
    // library, they may not use precisely the same types: for
    // example, they will probably each declare their own structs,
    // which are distinct types from LLVM's point of view (nominal
    // types).
    //
    // Now, if those two crates are linked into an application, and
    // they contain inlined code, you can wind up with a situation
    // where both of those functions wind up being loaded into this
    // application simultaneously. In that case, the same function
    // (from LLVM's point of view) requires two types. But of course
    // LLVM won't allow one function to have two types.
    //
    // What we currently do, therefore, is declare the function with
    // one of the two types (whichever happens to come first) and then
    // bitcast as needed when the function is referenced to make sure
    // it has the type we expect.
    //
    // This can occur on either a crate-local or crate-external
    // reference. It also occurs when testing libcore and in some
    // other weird situations. Annoying.
    let llptrty = type_of::type_of(ccx, datum.ty);
    if common::val_ty(datum.val) != llptrty {
        debug!("trans_fn_ref_with_substs(): casting pointer!");
        datum.val = consts::ptrcast(datum.val, llptrty);
    } else {
        debug!("trans_fn_ref_with_substs(): not casting pointer!");
    }

    datum
}

// ______________________________________________________________________
// Translating calls

fn trans_call_inner<'a, 'blk, 'tcx>(mut bcx: Block<'blk, 'tcx>,
                                    debug_loc: DebugLoc,
                                    callee: Callee<'tcx>,
                                    args: CallArgs<'a, 'tcx>,
                                    dest: Option<expr::Dest>)
                                    -> Result<'blk, 'tcx> {
    // Introduce a temporary cleanup scope that will contain cleanups
    // for the arguments while they are being evaluated. The purpose
    // this cleanup is to ensure that, should a panic occur while
    // evaluating argument N, the values for arguments 0...N-1 are all
    // cleaned up. If no panic occurs, the values are handed off to
    // the callee, and hence none of the cleanups in this temporary
    // scope will ever execute.
    let fcx = bcx.fcx;
    let ccx = fcx.ccx;

    let (abi, ret_ty) = match callee.ty.sty {
        ty::TyFnDef(_, _, ref f) | ty::TyFnPtr(ref f) => {
            let sig = bcx.tcx().erase_late_bound_regions(&f.sig);
            let sig = infer::normalize_associated_type(bcx.tcx(), &sig);
            (f.abi, sig.output)
        }
        _ => panic!("expected fn item or ptr in Callee::call")
    };

    match callee.data {
        Intrinsic(node, substs) => {
            assert!(abi == Abi::RustIntrinsic || abi == Abi::PlatformIntrinsic);
            assert!(dest.is_some());

            let call_info = match debug_loc {
                DebugLoc::At(id, span) => NodeIdAndSpan { id: id, span: span },
                DebugLoc::None => {
                    bcx.sess().bug("No call info for intrinsic call?")
                }
            };

            let arg_cleanup_scope = fcx.push_custom_cleanup_scope();
            return intrinsic::trans_intrinsic_call(bcx, node, callee.ty,
                                                   arg_cleanup_scope, args,
                                                   dest.unwrap(),
                                                   substs,
                                                   call_info);
        }
        NamedTupleConstructor(disr) => {
            assert!(dest.is_some());

            return base::trans_named_tuple_constructor(bcx,
                                                       callee.ty,
                                                       disr,
                                                       args,
                                                       dest.unwrap(),
                                                       debug_loc);
        }
        _ => {}
    }

    // Intrinsics should not become actual functions.
    // We trans them in place in `trans_intrinsic_call`
    assert!(abi != Abi::RustIntrinsic && abi != Abi::PlatformIntrinsic);

    let is_rust_fn = abi == Abi::Rust || abi == Abi::RustCall;

    // Generate a location to store the result. If the user does
    // not care about the result, just make a stack slot.
    let opt_llretslot = dest.and_then(|dest| match dest {
        expr::SaveIn(dst) => Some(dst),
        expr::Ignore => {
            let ret_ty = match ret_ty {
                ty::FnConverging(ret_ty) => ret_ty,
                ty::FnDiverging => ccx.tcx().mk_nil()
            };
            if !is_rust_fn ||
              type_of::return_uses_outptr(ccx, ret_ty) ||
              bcx.fcx.type_needs_drop(ret_ty) {
                // Push the out-pointer if we use an out-pointer for this
                // return type, otherwise push "undef".
                if common::type_is_zero_size(ccx, ret_ty) {
                    let llty = type_of::type_of(ccx, ret_ty);
                    Some(common::C_undef(llty.ptr_to()))
                } else {
                    let llresult = alloc_ty(bcx, ret_ty, "__llret");
                    call_lifetime_start(bcx, llresult);
                    Some(llresult)
                }
            } else {
                None
            }
        }
    });

    let mut llresult = unsafe {
        llvm::LLVMGetUndef(Type::nil(ccx).ptr_to().to_ref())
    };

    let arg_cleanup_scope = fcx.push_custom_cleanup_scope();

    // The code below invokes the function, using either the Rust
    // conventions (if it is a rust fn) or the native conventions
    // (otherwise).  The important part is that, when all is said
    // and done, either the return value of the function will have been
    // written in opt_llretslot (if it is Some) or `llresult` will be
    // set appropriately (otherwise).
    if is_rust_fn {
        let mut llargs = Vec::new();

        if let (ty::FnConverging(ret_ty), Some(mut llretslot)) = (ret_ty, opt_llretslot) {
            if type_of::return_uses_outptr(ccx, ret_ty) {
                let llformal_ret_ty = type_of::type_of(ccx, ret_ty).ptr_to();
                let llret_ty = common::val_ty(llretslot);
                if llformal_ret_ty != llret_ty {
                    // this could happen due to e.g. subtyping
                    debug!("casting actual return type ({:?}) to match formal ({:?})",
                        llret_ty, llformal_ret_ty);
                    llretslot = PointerCast(bcx, llretslot, llformal_ret_ty);
                }
                llargs.push(llretslot);
            }
        }

        let arg_start = llargs.len();

        // Push the arguments.
        bcx = trans_args(bcx,
                         args,
                         callee.ty,
                         &mut llargs,
                         cleanup::CustomScope(arg_cleanup_scope),
                         abi);

        fcx.scopes.borrow_mut().last_mut().unwrap().drop_non_lifetime_clean();

        let datum = match callee.data {
            Fn(f) => immediate_rvalue(f, callee.ty),
            Virtual(idx) => {
                // The data and vtable pointers were split by trans_arg_datum.
                let vtable = llargs.remove(arg_start + 1);
                meth::get_virtual_method(bcx, vtable, idx, callee.ty)
            }
            _ => unreachable!()
        };

        // Invoke the actual rust fn and update bcx/llresult.
        let (llret, b) = base::invoke(bcx,
                                      datum.val,
                                      &llargs[..],
                                      datum.ty,
                                      debug_loc);
        bcx = b;
        llresult = llret;

        // If the Rust convention for this type is return via
        // the return value, copy it into llretslot.
        match (opt_llretslot, ret_ty) {
            (Some(llretslot), ty::FnConverging(ret_ty)) => {
                if !type_of::return_uses_outptr(bcx.ccx(), ret_ty) &&
                    !common::type_is_zero_size(bcx.ccx(), ret_ty)
                {
                    store_ty(bcx, llret, llretslot, ret_ty)
                }
            }
            (_, _) => {}
        }
    } else {
        // Lang items are the only case where dest is None, and
        // they are always Rust fns.
        assert!(dest.is_some());

        let mut llargs = Vec::new();
        let (llfn, arg_tys) = match (callee.data, &args) {
            (Fn(f), &ArgExprs(a)) => {
                (f, a.iter().map(|x| common::expr_ty_adjusted(bcx, &x)).collect())
            }
            _ => panic!("expected fn ptr and arg exprs.")
        };
        bcx = trans_args(bcx,
                         args,
                         callee.ty,
                         &mut llargs,
                         cleanup::CustomScope(arg_cleanup_scope),
                         abi);
        fcx.scopes.borrow_mut().last_mut().unwrap().drop_non_lifetime_clean();

        bcx = foreign::trans_native_call(bcx,
                                         callee.ty,
                                         llfn,
                                         opt_llretslot.unwrap(),
                                         &llargs[..],
                                         arg_tys,
                                         debug_loc);
    }

    fcx.pop_and_trans_custom_cleanup_scope(bcx, arg_cleanup_scope);

    // If the caller doesn't care about the result of this fn call,
    // drop the temporary slot we made.
    match (dest, opt_llretslot, ret_ty) {
        (Some(expr::Ignore), Some(llretslot), ty::FnConverging(ret_ty)) => {
            // drop the value if it is not being saved.
            bcx = glue::drop_ty(bcx,
                                llretslot,
                                ret_ty,
                                debug_loc);
            call_lifetime_end(bcx, llretslot);
        }
        _ => {}
    }

    if ret_ty == ty::FnDiverging {
        Unreachable(bcx);
    }

    Result::new(bcx, llresult)
}

pub enum CallArgs<'a, 'tcx> {
    /// Supply value of arguments as a list of expressions that must be
    /// translated. This is used in the common case of `foo(bar, qux)`.
    ArgExprs(&'a [P<hir::Expr>]),

    /// Supply value of arguments as a list of LLVM value refs; frequently
    /// used with lang items and so forth, when the argument is an internal
    /// value.
    ArgVals(&'a [ValueRef]),

    /// For overloaded operators: `(lhs, Option(rhs))`.
    /// `lhs` is the left-hand-side and `rhs` is the datum
    /// of the right-hand-side argument (if any).
    ArgOverloadedOp(Datum<'tcx, Expr>, Option<Datum<'tcx, Expr>>),

    /// Supply value of arguments as a list of expressions that must be
    /// translated, for overloaded call operators.
    ArgOverloadedCall(Vec<&'a hir::Expr>),
}

fn trans_args_under_call_abi<'blk, 'tcx>(
                             mut bcx: Block<'blk, 'tcx>,
                             arg_exprs: &[P<hir::Expr>],
                             fn_ty: Ty<'tcx>,
                             llargs: &mut Vec<ValueRef>,
                             arg_cleanup_scope: cleanup::ScopeId)
                             -> Block<'blk, 'tcx>
{
    let sig = bcx.tcx().erase_late_bound_regions(&fn_ty.fn_sig());
    let sig = infer::normalize_associated_type(bcx.tcx(), &sig);
    let args = sig.inputs;

    // Translate the `self` argument first.
    let arg_datum = unpack_datum!(bcx, expr::trans(bcx, &arg_exprs[0]));
    bcx = trans_arg_datum(bcx,
                          args[0],
                          arg_datum,
                          arg_cleanup_scope,
                          llargs);

    // Now untuple the rest of the arguments.
    let tuple_expr = &arg_exprs[1];
    let tuple_type = common::node_id_type(bcx, tuple_expr.id);

    match tuple_type.sty {
        ty::TyTuple(ref field_types) => {
            let tuple_datum = unpack_datum!(bcx,
                                            expr::trans(bcx, &tuple_expr));
            let tuple_lvalue_datum =
                unpack_datum!(bcx,
                              tuple_datum.to_lvalue_datum(bcx,
                                                          "args",
                                                          tuple_expr.id));
            let repr = adt::represent_type(bcx.ccx(), tuple_type);
            let repr_ptr = &repr;
            for (i, field_type) in field_types.iter().enumerate() {
                let arg_datum = tuple_lvalue_datum.get_element(
                    bcx,
                    field_type,
                    |srcval| {
                        adt::trans_field_ptr(bcx, repr_ptr, srcval, Disr(0), i)
                    }).to_expr_datum();
                bcx = trans_arg_datum(bcx,
                                      field_type,
                                      arg_datum,
                                      arg_cleanup_scope,
                                      llargs);
            }
        }
        _ => {
            bcx.sess().span_bug(tuple_expr.span,
                                "argument to `.call()` wasn't a tuple?!")
        }
    };

    bcx
}

fn trans_overloaded_call_args<'blk, 'tcx>(
                              mut bcx: Block<'blk, 'tcx>,
                              arg_exprs: Vec<&hir::Expr>,
                              fn_ty: Ty<'tcx>,
                              llargs: &mut Vec<ValueRef>,
                              arg_cleanup_scope: cleanup::ScopeId)
                              -> Block<'blk, 'tcx> {
    // Translate the `self` argument first.
    let sig = bcx.tcx().erase_late_bound_regions(&fn_ty.fn_sig());
    let sig = infer::normalize_associated_type(bcx.tcx(), &sig);
    let arg_tys = sig.inputs;

    let arg_datum = unpack_datum!(bcx, expr::trans(bcx, arg_exprs[0]));
    bcx = trans_arg_datum(bcx,
                          arg_tys[0],
                          arg_datum,
                          arg_cleanup_scope,
                          llargs);

    // Now untuple the rest of the arguments.
    let tuple_type = arg_tys[1];
    match tuple_type.sty {
        ty::TyTuple(ref field_types) => {
            for (i, &field_type) in field_types.iter().enumerate() {
                let arg_datum =
                    unpack_datum!(bcx, expr::trans(bcx, arg_exprs[i + 1]));
                bcx = trans_arg_datum(bcx,
                                      field_type,
                                      arg_datum,
                                      arg_cleanup_scope,
                                      llargs);
            }
        }
        _ => {
            bcx.sess().span_bug(arg_exprs[0].span,
                                "argument to `.call()` wasn't a tuple?!")
        }
    };

    bcx
}

pub fn trans_args<'a, 'blk, 'tcx>(cx: Block<'blk, 'tcx>,
                                  args: CallArgs<'a, 'tcx>,
                                  fn_ty: Ty<'tcx>,
                                  llargs: &mut Vec<ValueRef>,
                                  arg_cleanup_scope: cleanup::ScopeId,
                                  abi: Abi)
                                  -> Block<'blk, 'tcx> {
    debug!("trans_args(abi={})", abi);

    let _icx = push_ctxt("trans_args");
    let sig = cx.tcx().erase_late_bound_regions(&fn_ty.fn_sig());
    let sig = infer::normalize_associated_type(cx.tcx(), &sig);
    let arg_tys = sig.inputs;
    let variadic = sig.variadic;

    let mut bcx = cx;

    // First we figure out the caller's view of the types of the arguments.
    // This will be needed if this is a generic call, because the callee has
    // to cast her view of the arguments to the caller's view.
    match args {
        ArgExprs(arg_exprs) => {
            if abi == Abi::RustCall {
                // This is only used for direct calls to the `call`,
                // `call_mut` or `call_once` functions.
                return trans_args_under_call_abi(cx,
                                                 arg_exprs,
                                                 fn_ty,
                                                 llargs,
                                                 arg_cleanup_scope)
            }

            let num_formal_args = arg_tys.len();
            for (i, arg_expr) in arg_exprs.iter().enumerate() {
                let arg_ty = if i >= num_formal_args {
                    assert!(variadic);
                    common::expr_ty_adjusted(cx, &arg_expr)
                } else {
                    arg_tys[i]
                };

                let arg_datum = unpack_datum!(bcx, expr::trans(bcx, &arg_expr));
                bcx = trans_arg_datum(bcx, arg_ty, arg_datum,
                                      arg_cleanup_scope,
                                      llargs);
            }
        }
        ArgOverloadedCall(arg_exprs) => {
            return trans_overloaded_call_args(cx,
                                              arg_exprs,
                                              fn_ty,
                                              llargs,
                                              arg_cleanup_scope)
        }
        ArgOverloadedOp(lhs, rhs) => {
            assert!(!variadic);

            bcx = trans_arg_datum(bcx, arg_tys[0], lhs,
                                  arg_cleanup_scope,
                                  llargs);

            if let Some(rhs) = rhs {
                assert_eq!(arg_tys.len(), 2);
                bcx = trans_arg_datum(bcx, arg_tys[1], rhs,
                                      arg_cleanup_scope,
                                      llargs);
            } else {
                assert_eq!(arg_tys.len(), 1);
            }
        }
        ArgVals(vs) => {
            llargs.extend_from_slice(vs);
        }
    }

    bcx
}

pub fn trans_arg_datum<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                                   formal_arg_ty: Ty<'tcx>,
                                   arg_datum: Datum<'tcx, Expr>,
                                   arg_cleanup_scope: cleanup::ScopeId,
                                   llargs: &mut Vec<ValueRef>)
                                   -> Block<'blk, 'tcx> {
    let _icx = push_ctxt("trans_arg_datum");
    let mut bcx = bcx;
    let ccx = bcx.ccx();

    debug!("trans_arg_datum({:?})", formal_arg_ty);

    let arg_datum_ty = arg_datum.ty;

    debug!("   arg datum: {:?}", arg_datum);

    let mut val = if common::type_is_fat_ptr(bcx.tcx(), arg_datum_ty) &&
                     !bcx.fcx.type_needs_drop(arg_datum_ty) {
        arg_datum.val
    } else {
        // Make this an rvalue, since we are going to be
        // passing ownership.
        let arg_datum = unpack_datum!(
            bcx, arg_datum.to_rvalue_datum(bcx, "arg"));

        // Now that arg_datum is owned, get it into the appropriate
        // mode (ref vs value).
        let arg_datum = unpack_datum!(
            bcx, arg_datum.to_appropriate_datum(bcx));

        // Technically, ownership of val passes to the callee.
        // However, we must cleanup should we panic before the
        // callee is actually invoked.
        arg_datum.add_clean(bcx.fcx, arg_cleanup_scope)
    };

    if type_of::arg_is_indirect(ccx, formal_arg_ty) && formal_arg_ty != arg_datum_ty {
        // this could happen due to e.g. subtyping
        let llformal_arg_ty = type_of::type_of_explicit_arg(ccx, formal_arg_ty);
        debug!("casting actual type ({:?}) to match formal ({:?})",
               Value(val), llformal_arg_ty);
        debug!("Rust types: {:?}; {:?}", arg_datum_ty,
                                     formal_arg_ty);
        val = PointerCast(bcx, val, llformal_arg_ty);
    }

    debug!("--- trans_arg_datum passing {:?}", Value(val));

    if common::type_is_fat_ptr(bcx.tcx(), formal_arg_ty) {
        llargs.push(Load(bcx, expr::get_dataptr(bcx, val)));
        llargs.push(Load(bcx, expr::get_meta(bcx, val)));
    } else {
        llargs.push(val);
    }

    bcx
}
