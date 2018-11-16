// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use llvm::{self, LLVMConstInBoundsGEP};
use rustc::ty::{self, Ty};
use rustc::ty::layout::{self, Align, TyLayout, LayoutOf, Size, VariantIdx};
use rustc::mir;
use rustc::mir::tcx::PlaceTy;
use base;
use builder::{Builder, MemFlags};
use common::{CodegenCx, IntPredicate};
use type_of::LayoutLlvmExt;
use value::Value;
use glue;
use mir::constant::const_alloc_to_llvm;

use interfaces::{
    BuilderMethods, ConstMethods, BaseTypeMethods, DerivedTypeMethods, DerivedIntrinsicMethods,
    StaticMethods,
};

use super::{FunctionCx, LocalRef};
use super::operand::{OperandRef, OperandValue};

#[derive(Copy, Clone, Debug)]
pub struct PlaceRef<'tcx, V> {
    /// Pointer to the contents of the place
    pub llval: V,

    /// This place's extra data if it is unsized, or null
    pub llextra: Option<V>,

    /// Monomorphized type of this place, including variant information
    pub layout: TyLayout<'tcx>,

    /// What alignment we know for this place
    pub align: Align,
}

impl PlaceRef<'tcx, &'ll Value> {
    pub fn new_sized(
        llval: &'ll Value,
        layout: TyLayout<'tcx>,
        align: Align,
    ) -> PlaceRef<'tcx, &'ll Value> {
        assert!(!layout.is_unsized());
        PlaceRef {
            llval,
            llextra: None,
            layout,
            align
        }
    }

    pub fn from_const_alloc(
        bx: &Builder<'a, 'll, 'tcx>,
        layout: TyLayout<'tcx>,
        alloc: &mir::interpret::Allocation,
        offset: Size,
    ) -> PlaceRef<'tcx, &'ll Value> {
        let init = const_alloc_to_llvm(bx.cx(), alloc);
        let base_addr = bx.cx().static_addr_of(init, layout.align, None);

        let llval = unsafe { LLVMConstInBoundsGEP(
            bx.cx().static_bitcast(base_addr, bx.cx().type_i8p()),
            &bx.cx().const_usize(offset.bytes()),
            1,
        )};
        let llval = bx.cx().static_bitcast(llval, bx.cx().type_ptr_to(layout.llvm_type(bx.cx())));
        PlaceRef::new_sized(llval, layout, alloc.align)
    }

    pub fn alloca(bx: &Builder<'a, 'll, 'tcx>, layout: TyLayout<'tcx>, name: &str)
                  -> PlaceRef<'tcx, &'ll Value> {
        debug!("alloca({:?}: {:?})", name, layout);
        assert!(!layout.is_unsized(), "tried to statically allocate unsized place");
        let tmp = bx.alloca(layout.llvm_type(bx.cx()), name, layout.align);
        Self::new_sized(tmp, layout, layout.align)
    }

    /// Returns a place for an indirect reference to an unsized place.
    pub fn alloca_unsized_indirect(
        bx: &Builder<'a, 'll, 'tcx>,
        layout: TyLayout<'tcx>,
        name: &str,
    ) -> PlaceRef<'tcx, &'ll Value> {
        debug!("alloca_unsized_indirect({:?}: {:?})", name, layout);
        assert!(layout.is_unsized(), "tried to allocate indirect place for sized values");
        let ptr_ty = bx.cx().tcx.mk_mut_ptr(layout.ty);
        let ptr_layout = bx.cx().layout_of(ptr_ty);
        Self::alloca(bx, ptr_layout, name)
    }

    pub fn len(&self, cx: &CodegenCx<'ll, 'tcx>) -> &'ll Value {
        if let layout::FieldPlacement::Array { count, .. } = self.layout.fields {
            if self.layout.is_unsized() {
                assert_eq!(count, 0);
                self.llextra.unwrap()
            } else {
                cx.const_usize(count)
            }
        } else {
            bug!("unexpected layout `{:#?}` in PlaceRef::len", self.layout)
        }
    }

    pub fn load(&self, bx: &Builder<'a, 'll, 'tcx>) -> OperandRef<'tcx, &'ll Value> {
        debug!("PlaceRef::load: {:?}", self);

        assert_eq!(self.llextra.is_some(), self.layout.is_unsized());

        if self.layout.is_zst() {
            return OperandRef::new_zst(bx.cx(), self.layout);
        }

        let scalar_load_metadata = |load, scalar: &layout::Scalar| {
            let vr = scalar.valid_range.clone();
            match scalar.value {
                layout::Int(..) => {
                    let range = scalar.valid_range_exclusive(bx.cx());
                    if range.start != range.end {
                        bx.range_metadata(load, range);
                    }
                }
                layout::Pointer if vr.start() < vr.end() && !vr.contains(&0) => {
                    bx.nonnull_metadata(load);
                }
                _ => {}
            }
        };

        let val = if let Some(llextra) = self.llextra {
            OperandValue::Ref(self.llval, Some(llextra), self.align)
        } else if self.layout.is_llvm_immediate() {
            let mut const_llval = None;
            unsafe {
                if let Some(global) = llvm::LLVMIsAGlobalVariable(self.llval) {
                    if llvm::LLVMIsGlobalConstant(global) == llvm::True {
                        const_llval = llvm::LLVMGetInitializer(global);
                    }
                }
            }
            let llval = const_llval.unwrap_or_else(|| {
                let load = bx.load(self.llval, self.align);
                if let layout::Abi::Scalar(ref scalar) = self.layout.abi {
                    scalar_load_metadata(load, scalar);
                }
                load
            });
            OperandValue::Immediate(base::to_immediate(bx, llval, self.layout))
        } else if let layout::Abi::ScalarPair(ref a, ref b) = self.layout.abi {
            let load = |i, scalar: &layout::Scalar| {
                let llptr = bx.struct_gep(self.llval, i as u64);
                let load = bx.load(llptr, self.align);
                scalar_load_metadata(load, scalar);
                if scalar.is_bool() {
                    bx.trunc(load, bx.cx().type_i1())
                } else {
                    load
                }
            };
            OperandValue::Pair(load(0, a), load(1, b))
        } else {
            OperandValue::Ref(self.llval, None, self.align)
        };

        OperandRef { val, layout: self.layout }
    }

    /// Access a field, at a point when the value's case is known.
    pub fn project_field(
        self,
        bx: &Builder<'a, 'll, 'tcx>,
        ix: usize,
    ) -> PlaceRef<'tcx, &'ll Value> {
        let cx = bx.cx();
        let field = self.layout.field(cx, ix);
        let offset = self.layout.fields.offset(ix);
        let effective_field_align = self.align.restrict_for_offset(offset);

        let simple = || {
            // Unions and newtypes only use an offset of 0.
            let llval = if offset.bytes() == 0 {
                self.llval
            } else if let layout::Abi::ScalarPair(ref a, ref b) = self.layout.abi {
                // Offsets have to match either first or second field.
                assert_eq!(offset, a.value.size(cx).abi_align(b.value.align(cx)));
                bx.struct_gep(self.llval, 1)
            } else {
                bx.struct_gep(self.llval, self.layout.llvm_field_index(ix))
            };
            PlaceRef {
                // HACK(eddyb) have to bitcast pointers until LLVM removes pointee types.
                llval: bx.pointercast(llval, cx.type_ptr_to(field.llvm_type(cx))),
                llextra: if cx.type_has_metadata(field.ty) {
                    self.llextra
                } else {
                    None
                },
                layout: field,
                align: effective_field_align,
            }
        };

        // Simple cases, which don't need DST adjustment:
        //   * no metadata available - just log the case
        //   * known alignment - sized types, [T], str or a foreign type
        //   * packed struct - there is no alignment padding
        match field.ty.sty {
            _ if self.llextra.is_none() => {
                debug!("Unsized field `{}`, of `{:?}` has no metadata for adjustment",
                    ix, self.llval);
                return simple();
            }
            _ if !field.is_unsized() => return simple(),
            ty::Slice(..) | ty::Str | ty::Foreign(..) => return simple(),
            ty::Adt(def, _) => {
                if def.repr.packed() {
                    // FIXME(eddyb) generalize the adjustment when we
                    // start supporting packing to larger alignments.
                    assert_eq!(self.layout.align.abi(), 1);
                    return simple();
                }
            }
            _ => {}
        }

        // We need to get the pointer manually now.
        // We do this by casting to a *i8, then offsetting it by the appropriate amount.
        // We do this instead of, say, simply adjusting the pointer from the result of a GEP
        // because the field may have an arbitrary alignment in the LLVM representation
        // anyway.
        //
        // To demonstrate:
        //   struct Foo<T: ?Sized> {
        //      x: u16,
        //      y: T
        //   }
        //
        // The type Foo<Foo<Trait>> is represented in LLVM as { u16, { u16, u8 }}, meaning that
        // the `y` field has 16-bit alignment.

        let meta = self.llextra;

        let unaligned_offset = cx.const_usize(offset.bytes());

        // Get the alignment of the field
        let (_, unsized_align) = glue::size_and_align_of_dst(bx, field.ty, meta);

        // Bump the unaligned offset up to the appropriate alignment using the
        // following expression:
        //
        //   (unaligned offset + (align - 1)) & -align

        // Calculate offset
        let align_sub_1 = bx.sub(unsized_align, cx.const_usize(1u64));
        let offset = bx.and(bx.add(unaligned_offset, align_sub_1),
        bx.neg(unsized_align));

        debug!("struct_field_ptr: DST field offset: {:?}", offset);

        // Cast and adjust pointer
        let byte_ptr = bx.pointercast(self.llval, cx.type_i8p());
        let byte_ptr = bx.gep(byte_ptr, &[offset]);

        // Finally, cast back to the type expected
        let ll_fty = field.llvm_type(cx);
        debug!("struct_field_ptr: Field type is {:?}", ll_fty);

        PlaceRef {
            llval: bx.pointercast(byte_ptr, bx.cx().type_ptr_to(ll_fty)),
            llextra: self.llextra,
            layout: field,
            align: effective_field_align,
        }
    }

    /// Obtain the actual discriminant of a value.
    pub fn codegen_get_discr(
        self,
        bx: &Builder<'a, 'll, 'tcx>,
        cast_to: Ty<'tcx>
    ) -> &'ll Value {
        let cast_to = bx.cx().layout_of(cast_to).immediate_llvm_type(bx.cx());
        if self.layout.abi.is_uninhabited() {
            return bx.cx().const_undef(cast_to);
        }
        match self.layout.variants {
            layout::Variants::Single { index } => {
                let discr_val = self.layout.ty.ty_adt_def().map_or(
                    index.as_u32() as u128,
                    |def| def.discriminant_for_variant(bx.cx().tcx, index).val);
                return bx.cx().const_uint_big(cast_to, discr_val);
            }
            layout::Variants::Tagged { .. } |
            layout::Variants::NicheFilling { .. } => {},
        }

        let discr = self.project_field(bx, 0);
        let lldiscr = discr.load(bx).immediate();
        match self.layout.variants {
            layout::Variants::Single { .. } => bug!(),
            layout::Variants::Tagged { ref tag, .. } => {
                let signed = match tag.value {
                    // We use `i1` for bytes that are always `0` or `1`,
                    // e.g. `#[repr(i8)] enum E { A, B }`, but we can't
                    // let LLVM interpret the `i1` as signed, because
                    // then `i1 1` (i.e. E::B) is effectively `i8 -1`.
                    layout::Int(_, signed) => !tag.is_bool() && signed,
                    _ => false
                };
                bx.intcast(lldiscr, cast_to, signed)
            }
            layout::Variants::NicheFilling {
                dataful_variant,
                ref niche_variants,
                niche_start,
                ..
            } => {
                let niche_llty = discr.layout.immediate_llvm_type(bx.cx());
                if niche_variants.start() == niche_variants.end() {
                    // FIXME(eddyb) Check the actual primitive type here.
                    let niche_llval = if niche_start == 0 {
                        // HACK(eddyb) Using `c_null` as it works on all types.
                        bx.cx().const_null(niche_llty)
                    } else {
                        bx.cx().const_uint_big(niche_llty, niche_start)
                    };
                    bx.select(bx.icmp(IntPredicate::IntEQ, lldiscr, niche_llval),
                        bx.cx().const_uint(cast_to, niche_variants.start().as_u32() as u64),
                        bx.cx().const_uint(cast_to, dataful_variant.as_u32() as u64))
                } else {
                    // Rebase from niche values to discriminant values.
                    let delta = niche_start.wrapping_sub(niche_variants.start().as_u32() as u128);
                    let lldiscr = bx.sub(lldiscr, bx.cx().const_uint_big(niche_llty, delta));
                    let lldiscr_max =
                        bx.cx().const_uint(niche_llty, niche_variants.end().as_u32() as u64);
                    bx.select(bx.icmp(IntPredicate::IntULE, lldiscr, lldiscr_max),
                        bx.intcast(lldiscr, cast_to, false),
                        bx.cx().const_uint(cast_to, dataful_variant.as_u32() as u64))
                }
            }
        }
    }

    /// Set the discriminant for a new value of the given case of the given
    /// representation.
    pub fn codegen_set_discr(&self, bx: &Builder<'a, 'll, 'tcx>, variant_index: VariantIdx) {
        if self.layout.for_variant(bx.cx(), variant_index).abi.is_uninhabited() {
            return;
        }
        match self.layout.variants {
            layout::Variants::Single { index } => {
                assert_eq!(index, variant_index);
            }
            layout::Variants::Tagged { .. } => {
                let ptr = self.project_field(bx, 0);
                let to = self.layout.ty.ty_adt_def().unwrap()
                    .discriminant_for_variant(bx.tcx(), variant_index)
                    .val;
                bx.store(
                    bx.cx().const_uint_big(ptr.layout.llvm_type(bx.cx()), to),
                    ptr.llval,
                    ptr.align);
            }
            layout::Variants::NicheFilling {
                dataful_variant,
                ref niche_variants,
                niche_start,
                ..
            } => {
                if variant_index != dataful_variant {
                    if bx.sess().target.target.arch == "arm" ||
                       bx.sess().target.target.arch == "aarch64" {
                        // Issue #34427: As workaround for LLVM bug on ARM,
                        // use memset of 0 before assigning niche value.
                        let fill_byte = bx.cx().const_u8(0);
                        let (size, align) = self.layout.size_and_align();
                        let size = bx.cx().const_usize(size.bytes());
                        bx.memset(self.llval, fill_byte, size, align, MemFlags::empty());
                    }

                    let niche = self.project_field(bx, 0);
                    let niche_llty = niche.layout.immediate_llvm_type(bx.cx());
                    let niche_value = variant_index.as_u32() - niche_variants.start().as_u32();
                    let niche_value = (niche_value as u128)
                        .wrapping_add(niche_start);
                    // FIXME(eddyb) Check the actual primitive type here.
                    let niche_llval = if niche_value == 0 {
                        // HACK(eddyb) Using `c_null` as it works on all types.
                        bx.cx().const_null(niche_llty)
                    } else {
                        bx.cx().const_uint_big(niche_llty, niche_value)
                    };
                    OperandValue::Immediate(niche_llval).store(bx, niche);
                }
            }
        }
    }

    pub fn project_index(&self, bx: &Builder<'a, 'll, 'tcx>, llindex: &'ll Value)
                         -> PlaceRef<'tcx, &'ll Value> {
        PlaceRef {
            llval: bx.inbounds_gep(self.llval, &[bx.cx().const_usize(0), llindex]),
            llextra: None,
            layout: self.layout.field(bx.cx(), 0),
            align: self.align
        }
    }

    pub fn project_downcast(&self, bx: &Builder<'a, 'll, 'tcx>, variant_index: VariantIdx)
                            -> PlaceRef<'tcx, &'ll Value> {
        let mut downcast = *self;
        downcast.layout = self.layout.for_variant(bx.cx(), variant_index);

        // Cast to the appropriate variant struct type.
        let variant_ty = downcast.layout.llvm_type(bx.cx());
        downcast.llval = bx.pointercast(downcast.llval, bx.cx().type_ptr_to(variant_ty));

        downcast
    }

    pub fn storage_live(&self, bx: &Builder<'a, 'll, 'tcx>) {
        bx.lifetime_start(self.llval, self.layout.size);
    }

    pub fn storage_dead(&self, bx: &Builder<'a, 'll, 'tcx>) {
        bx.lifetime_end(self.llval, self.layout.size);
    }
}

impl FunctionCx<'a, 'll, 'tcx, &'ll Value> {
    pub fn codegen_place(&mut self,
                        bx: &Builder<'a, 'll, 'tcx>,
                        place: &mir::Place<'tcx>)
                        -> PlaceRef<'tcx, &'ll Value> {
        debug!("codegen_place(place={:?})", place);

        let cx = bx.cx();
        let tcx = cx.tcx;

        if let mir::Place::Local(index) = *place {
            match self.locals[index] {
                LocalRef::Place(place) => {
                    return place;
                }
                LocalRef::UnsizedPlace(place) => {
                    return place.load(bx).deref(&cx);
                }
                LocalRef::Operand(..) => {
                    bug!("using operand local {:?} as place", place);
                }
            }
        }

        let result = match *place {
            mir::Place::Local(_) => bug!(), // handled above
            mir::Place::Promoted(box (index, ty)) => {
                let param_env = ty::ParamEnv::reveal_all();
                let cid = mir::interpret::GlobalId {
                    instance: self.instance,
                    promoted: Some(index),
                };
                let layout = cx.layout_of(self.monomorphize(&ty));
                match bx.tcx().const_eval(param_env.and(cid)) {
                    Ok(val) => match val.val {
                        mir::interpret::ConstValue::ByRef(_, alloc, offset) => {
                            PlaceRef::from_const_alloc(bx, layout, alloc, offset)
                        }
                        _ => bug!("promoteds should have an allocation: {:?}", val),
                    },
                    Err(_) => {
                        // this is unreachable as long as runtime
                        // and compile-time agree on values
                        // With floats that won't always be true
                        // so we generate an abort
                        let fnname = bx.cx().get_intrinsic(&("llvm.trap"));
                        bx.call(fnname, &[], None);
                        let llval = bx.cx().const_undef(
                            bx.cx().type_ptr_to(layout.llvm_type(bx.cx()))
                        );
                        PlaceRef::new_sized(llval, layout, layout.align)
                    }
                }
            }
            mir::Place::Static(box mir::Static { def_id, ty }) => {
                let layout = cx.layout_of(self.monomorphize(&ty));
                PlaceRef::new_sized(cx.get_static(def_id), layout, layout.align)
            },
            mir::Place::Projection(box mir::Projection {
                ref base,
                elem: mir::ProjectionElem::Deref
            }) => {
                // Load the pointer from its location.
                self.codegen_consume(bx, base).deref(bx.cx())
            }
            mir::Place::Projection(ref projection) => {
                let cg_base = self.codegen_place(bx, &projection.base);

                match projection.elem {
                    mir::ProjectionElem::Deref => bug!(),
                    mir::ProjectionElem::Field(ref field, _) => {
                        cg_base.project_field(bx, field.index())
                    }
                    mir::ProjectionElem::Index(index) => {
                        let index = &mir::Operand::Copy(mir::Place::Local(index));
                        let index = self.codegen_operand(bx, index);
                        let llindex = index.immediate();
                        cg_base.project_index(bx, llindex)
                    }
                    mir::ProjectionElem::ConstantIndex { offset,
                                                         from_end: false,
                                                         min_length: _ } => {
                        let lloffset = bx.cx().const_usize(offset as u64);
                        cg_base.project_index(bx, lloffset)
                    }
                    mir::ProjectionElem::ConstantIndex { offset,
                                                         from_end: true,
                                                         min_length: _ } => {
                        let lloffset = bx.cx().const_usize(offset as u64);
                        let lllen = cg_base.len(bx.cx());
                        let llindex = bx.sub(lllen, lloffset);
                        cg_base.project_index(bx, llindex)
                    }
                    mir::ProjectionElem::Subslice { from, to } => {
                        let mut subslice = cg_base.project_index(bx,
                            bx.cx().const_usize(from as u64));
                        let projected_ty = PlaceTy::Ty { ty: cg_base.layout.ty }
                            .projection_ty(tcx, &projection.elem)
                            .to_ty(bx.tcx());
                        subslice.layout = bx.cx().layout_of(self.monomorphize(&projected_ty));

                        if subslice.layout.is_unsized() {
                            subslice.llextra = Some(bx.sub(cg_base.llextra.unwrap(),
                                bx.cx().const_usize((from as u64) + (to as u64))));
                        }

                        // Cast the place pointer type to the new
                        // array or slice type (*[%_; new_len]).
                        subslice.llval = bx.pointercast(subslice.llval,
                            bx.cx().type_ptr_to(subslice.layout.llvm_type(bx.cx())));

                        subslice
                    }
                    mir::ProjectionElem::Downcast(_, v) => {
                        cg_base.project_downcast(bx, v)
                    }
                }
            }
        };
        debug!("codegen_place(place={:?}) => {:?}", place, result);
        result
    }

    pub fn monomorphized_place_ty(&self, place: &mir::Place<'tcx>) -> Ty<'tcx> {
        let tcx = self.cx.tcx;
        let place_ty = place.ty(self.mir, tcx);
        self.monomorphize(&place_ty.to_ty(tcx))
    }
}
