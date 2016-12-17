// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use attributes;
use llvm::{ValueRef, get_params};
use rustc::traits;
use abi::FnType;
use callee::Callee;
use common::*;
use consts;
use declare;
use glue;
use machine;
use monomorphize::Instance;
use type_::Type;
use type_of::*;
use value::Value;
use rustc::ty;

// drop_glue pointer, size, align.
const VTABLE_OFFSET: usize = 3;

/// Extracts a method from a trait object's vtable, at the specified index.
pub fn get_virtual_method<'blk, 'tcx>(bcx: &BlockAndBuilder<'blk, 'tcx>,
                                      llvtable: ValueRef,
                                      vtable_index: usize)
                                      -> ValueRef {
    // Load the data pointer from the object.
    debug!("get_virtual_method(vtable_index={}, llvtable={:?})",
           vtable_index, Value(llvtable));

    bcx.load(bcx.gepi(llvtable, &[vtable_index + VTABLE_OFFSET]))
}

/// Generate a shim function that allows an object type like `SomeTrait` to
/// implement the type `SomeTrait`. Imagine a trait definition:
///
///    trait SomeTrait { fn get(&self) -> i32; ... }
///
/// And a generic bit of code:
///
///    fn foo<T:SomeTrait>(t: &T) {
///        let x = SomeTrait::get;
///        x(t)
///    }
///
/// What is the value of `x` when `foo` is invoked with `T=SomeTrait`?
/// The answer is that it is a shim function generated by this routine:
///
///    fn shim(t: &SomeTrait) -> i32 {
///        // ... call t.get() virtually ...
///    }
///
/// In fact, all virtual calls can be thought of as normal trait calls
/// that go through this shim function.
pub fn trans_object_shim<'a, 'tcx>(ccx: &'a CrateContext<'a, 'tcx>,
                                   callee: Callee<'tcx>)
                                   -> ValueRef {
    let tcx = ccx.tcx();

    debug!("trans_object_shim({:?})", callee);

    let (sig, abi, function_name) = match callee.ty.sty {
        ty::TyFnDef(def_id, substs, f) => {
            let instance = Instance::new(def_id, substs);
            (&f.sig, f.abi, instance.symbol_name(ccx.shared()))
        }
        _ => bug!()
    };

    let sig = tcx.erase_late_bound_regions_and_normalize(sig);
    let fn_ty = FnType::new(ccx, abi, &sig, &[]);

    let llfn = declare::define_internal_fn(ccx, &function_name, callee.ty);
    attributes::set_frame_pointer_elimination(ccx, llfn);

    let fcx = FunctionContext::new(ccx, llfn, fn_ty, None, false);
    let bcx = fcx.get_entry_block();

    let llargs = get_params(fcx.llfn);
    callee.call(&bcx, &llargs[fcx.fn_ty.ret.is_indirect() as usize..], fcx.llretslotptr, None);
    fcx.finish(&bcx);

    llfn
}

/// Creates a dynamic vtable for the given type and vtable origin.
/// This is used only for objects.
///
/// The vtables are cached instead of created on every call.
///
/// The `trait_ref` encodes the erased self type. Hence if we are
/// making an object `Foo<Trait>` from a value of type `Foo<T>`, then
/// `trait_ref` would map `T:Trait`.
pub fn get_vtable<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                            ty: ty::Ty<'tcx>,
                            trait_ref: Option<ty::PolyExistentialTraitRef<'tcx>>)
                            -> ValueRef
{
    let tcx = ccx.tcx();

    debug!("get_vtable(ty={:?}, trait_ref={:?})", ty, trait_ref);

    // Check the cache.
    if let Some(&val) = ccx.vtables().borrow().get(&(ty, trait_ref)) {
        return val;
    }

    // Not in the cache. Build it.
    let nullptr = C_null(Type::nil(ccx).ptr_to());

    let size_ty = sizing_type_of(ccx, ty);
    let size = machine::llsize_of_alloc(ccx, size_ty);
    let align = align_of(ccx, ty);

    let mut components: Vec<_> = [
        // Generate a destructor for the vtable.
        glue::get_drop_glue(ccx, ty),
        C_uint(ccx, size),
        C_uint(ccx, align)
    ].iter().cloned().collect();

    if let Some(trait_ref) = trait_ref {
        let trait_ref = trait_ref.with_self_ty(tcx, ty);
        let methods = traits::get_vtable_methods(tcx, trait_ref).map(|opt_mth| {
            opt_mth.map_or(nullptr, |(def_id, substs)| {
                Callee::def(ccx, def_id, substs).reify(ccx)
            })
        });
        components.extend(methods);
    }

    let vtable_const = C_struct(ccx, &components, false);
    let align = machine::llalign_of_pref(ccx, val_ty(vtable_const));
    let vtable = consts::addr_of(ccx, vtable_const, align, "vtable");

    ccx.vtables().borrow_mut().insert((ty, trait_ref), vtable);
    vtable
}
