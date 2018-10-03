
use crate::prelude::*;

pub fn codegen_intrinsic_call<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    def_id: DefId,
    substs: &'tcx Substs,
    args: &[CValue<'tcx>],
    destination: Option<(CPlace<'tcx>, BasicBlock)>,
) {
    let intrinsic = fx.tcx.item_name(def_id).as_str();
    let intrinsic = &intrinsic[..];

    let ret = match destination {
        Some((place, _)) => place,
        None => {
            // Insert non returning intrinsics here
            match intrinsic {
                "abort" => {
                    fx.bcx.ins().trap(TrapCode::User(!0 - 1));
                }
                "unreachable" => {
                    fx.bcx.ins().trap(TrapCode::User(!0 - 1));
                }
                _ => unimplemented!("unsupported instrinsic {}", intrinsic),
            }
            return;
        }
    };

    let u64_layout = fx.layout_of(fx.tcx.types.u64);
    let usize_layout = fx.layout_of(fx.tcx.types.usize);

    match intrinsic {
        "assume" => {
            assert_eq!(args.len(), 1);
        }
        "arith_offset" => {
            assert_eq!(args.len(), 2);
            let base = args[0].load_value(fx);
            let offset = args[1].load_value(fx);
            let res = fx.bcx.ins().iadd(base, offset);
            let res = CValue::ByVal(res, ret.layout());
            ret.write_cvalue(fx, res);
        }
        "likely" | "unlikely" => {
            assert_eq!(args.len(), 1);
            ret.write_cvalue(fx, args[0]);
        }
        "copy" | "copy_nonoverlapping" => {
            let elem_ty = substs.type_at(0);
            let elem_size: u64 = fx.layout_of(elem_ty).size.bytes();
            let elem_size = fx
                .bcx
                .ins()
                .iconst(fx.module.pointer_type(), elem_size as i64);
            assert_eq!(args.len(), 3);
            let src = args[0].load_value(fx);
            let dst = args[1].load_value(fx);
            let count = args[2].load_value(fx);
            let byte_amount = fx.bcx.ins().imul(count, elem_size);

            if intrinsic.ends_with("_nonoverlapping") {
                fx.bcx.call_memcpy(fx.isa, dst, src, byte_amount);
            } else {
                fx.bcx.call_memmove(fx.isa, dst, src, byte_amount);
            }
        }
        "discriminant_value" => {
            assert_eq!(args.len(), 1);
            let discr = crate::base::trans_get_discriminant(fx, args[0], ret.layout());
            ret.write_cvalue(fx, discr);
        }
        "size_of" => {
            assert_eq!(args.len(), 0);
            let size_of = fx.layout_of(substs.type_at(0)).size.bytes();
            let size_of = CValue::const_val(fx, usize_layout.ty, size_of as i64);
            ret.write_cvalue(fx, size_of);
        }
        "size_of_val" => {
            assert_eq!(args.len(), 1);
            let layout = fx.layout_of(substs.type_at(0));
            let size = match &layout.ty.sty {
                _ if !layout.is_unsized() => fx
                    .bcx
                    .ins()
                    .iconst(fx.module.pointer_type(), layout.size.bytes() as i64),
                ty::Slice(elem) => {
                    let len = args[0].load_value_pair(fx).1;
                    let elem_size = fx.layout_of(elem).size.bytes();
                    fx.bcx.ins().imul_imm(len, elem_size as i64)
                }
                ty::Dynamic(..) => crate::vtable::size_of_obj(fx, args[0]),
                ty => bug!("size_of_val for unknown unsized type {:?}", ty),
            };
            ret.write_cvalue(fx, CValue::ByVal(size, usize_layout));
        }
        "min_align_of" => {
            assert_eq!(args.len(), 0);
            let min_align = fx.layout_of(substs.type_at(0)).align.abi();
            let min_align = CValue::const_val(fx, usize_layout.ty, min_align as i64);
            ret.write_cvalue(fx, min_align);
        }
        "min_align_of_val" => {
            assert_eq!(args.len(), 1);
            let layout = fx.layout_of(substs.type_at(0));
            let align = match &layout.ty.sty {
                _ if !layout.is_unsized() => fx
                    .bcx
                    .ins()
                    .iconst(fx.module.pointer_type(), layout.align.abi() as i64),
                ty::Slice(elem) => {
                    let align = fx.layout_of(elem).align.abi() as i64;
                    fx.bcx.ins().iconst(fx.module.pointer_type(), align)
                }
                ty::Dynamic(..) => crate::vtable::min_align_of_obj(fx, args[0]),
                ty => unimplemented!("min_align_of_val for {:?}", ty),
            };
            ret.write_cvalue(fx, CValue::ByVal(align, usize_layout));
        }
        "type_id" => {
            assert_eq!(args.len(), 0);
            let type_id = fx.tcx.type_id_hash(substs.type_at(0));
            let type_id = CValue::const_val(fx, u64_layout.ty, type_id as i64);
            ret.write_cvalue(fx, type_id);
        }
        _ if intrinsic.starts_with("unchecked_") => {
            assert_eq!(args.len(), 2);
            let bin_op = match intrinsic {
                "unchecked_div" => BinOp::Div,
                "unchecked_rem" => BinOp::Rem,
                "unchecked_shl" => BinOp::Shl,
                "unchecked_shr" => BinOp::Shr,
                _ => unimplemented!("intrinsic {}", intrinsic),
            };
            let res = match ret.layout().ty.sty {
                ty::Uint(_) => crate::base::trans_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    false,
                ),
                ty::Int(_) => crate::base::trans_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    true,
                ),
                _ => panic!(),
            };
            ret.write_cvalue(fx, res);
        }
        _ if intrinsic.ends_with("_with_overflow") => {
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].layout().ty, args[1].layout().ty);
            let bin_op = match intrinsic {
                "add_with_overflow" => BinOp::Add,
                "sub_with_overflow" => BinOp::Sub,
                "mul_with_overflow" => BinOp::Mul,
                _ => unimplemented!("intrinsic {}", intrinsic),
            };
            let res = match args[0].layout().ty.sty {
                ty::Uint(_) => crate::base::trans_checked_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    false,
                ),
                ty::Int(_) => crate::base::trans_checked_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    true,
                ),
                _ => panic!(),
            };
            ret.write_cvalue(fx, res);
        }
        _ if intrinsic.starts_with("overflowing_") => {
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].layout().ty, args[1].layout().ty);
            let bin_op = match intrinsic {
                "overflowing_add" => BinOp::Add,
                "overflowing_sub" => BinOp::Sub,
                "overflowing_mul" => BinOp::Mul,
                _ => unimplemented!("intrinsic {}", intrinsic),
            };
            let res = match args[0].layout().ty.sty {
                ty::Uint(_) => crate::base::trans_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    false,
                ),
                ty::Int(_) => crate::base::trans_int_binop(
                    fx,
                    bin_op,
                    args[0],
                    args[1],
                    ret.layout().ty,
                    true,
                ),
                _ => panic!(),
            };
            ret.write_cvalue(fx, res);
        }
        "offset" => {
            assert_eq!(args.len(), 2);
            let base = args[0].load_value(fx);
            let offset = args[1].load_value(fx);
            let res = fx.bcx.ins().iadd(base, offset);
            ret.write_cvalue(fx, CValue::ByVal(res, args[0].layout()));
        }
        "transmute" => {
            assert_eq!(args.len(), 1);
            let src_ty = substs.type_at(0);
            let dst_ty = substs.type_at(1);
            assert_eq!(args[0].layout().ty, src_ty);
            let addr = args[0].force_stack(fx);
            let dst_layout = fx.layout_of(dst_ty);
            ret.write_cvalue(fx, CValue::ByRef(addr, dst_layout))
        }
        "init" => {
            assert_eq!(args.len(), 0);
            let ty = substs.type_at(0);
            let layout = fx.layout_of(ty);
            let stack_slot = fx.bcx.create_stack_slot(StackSlotData {
                kind: StackSlotKind::ExplicitSlot,
                size: layout.size.bytes() as u32,
                offset: None,
            });
            let addr = fx.bcx.ins().stack_addr(pointer_ty(fx.tcx), stack_slot, 0);
            let zero_val = fx.bcx.ins().iconst(types::I8, 0);
            let len_val = fx.bcx.ins().iconst(pointer_ty(fx.tcx), layout.size.bytes() as i64);
            fx.bcx.call_memset(fx.isa, addr, zero_val, len_val);

            let uninit_place = CPlace::from_stack_slot(fx, stack_slot, ty);
            let uninit_val = uninit_place.to_cvalue(fx);
            ret.write_cvalue(fx, uninit_val);
        }
        "uninit" => {
            assert_eq!(args.len(), 0);
            let ty = substs.type_at(0);
            let layout = fx.layout_of(ty);
            let stack_slot = fx.bcx.create_stack_slot(StackSlotData {
                kind: StackSlotKind::ExplicitSlot,
                size: layout.size.bytes() as u32,
                offset: None,
            });

            let uninit_place = CPlace::from_stack_slot(fx, stack_slot, ty);
            let uninit_val = uninit_place.to_cvalue(fx);
            ret.write_cvalue(fx, uninit_val);
        }
        "ctlz" | "ctlz_nonzero" => {
            assert_eq!(args.len(), 1);
            let arg = args[0].load_value(fx);
            let res = CValue::ByVal(fx.bcx.ins().clz(arg), args[0].layout());
            ret.write_cvalue(fx, res);
        }
        "cttz" | "cttz_nonzero" => {
            assert_eq!(args.len(), 1);
            let arg = args[0].load_value(fx);
            let res = CValue::ByVal(fx.bcx.ins().clz(arg), args[0].layout());
            ret.write_cvalue(fx, res);
        }
        "ctpop" => {
            assert_eq!(args.len(), 1);
            let arg = args[0].load_value(fx);
            let res = CValue::ByVal(fx.bcx.ins().popcnt(arg), args[0].layout());
            ret.write_cvalue(fx, res);
        }
        "bitreverse" => {
            assert_eq!(args.len(), 1);
            let arg = args[0].load_value(fx);
            let res = CValue::ByVal(fx.bcx.ins().bitrev(arg), args[0].layout());
            ret.write_cvalue(fx, res);
        }
        "needs_drop" => {
            assert_eq!(args.len(), 0);
            let ty = substs.type_at(0);
            let needs_drop = if ty.needs_drop(fx.tcx, ParamEnv::reveal_all()) {
                1
            } else {
                0
            };
            let needs_drop = CValue::const_val(fx, fx.tcx.types.bool, needs_drop);
            ret.write_cvalue(fx, needs_drop);
        }
        _ if intrinsic.starts_with("atomic_fence") => {}
        _ if intrinsic.starts_with("atomic_singlethreadfence") => {}
        _ if intrinsic.starts_with("atomic_load") => {
            assert_eq!(args.len(), 1);
            let inner_layout =
                fx.layout_of(args[0].layout().ty.builtin_deref(true).unwrap().ty);
            let val = CValue::ByRef(args[0].load_value(fx), inner_layout);
            ret.write_cvalue(fx, val);
        }
        _ if intrinsic.starts_with("atomic_store") => {
            assert_eq!(args.len(), 2);
            let dest = CPlace::Addr(args[0].load_value(fx), None, args[1].layout());
            dest.write_cvalue(fx, args[1]);
        }
        _ if intrinsic.starts_with("atomic_xadd") => {
            assert_eq!(args.len(), 2);
            let clif_ty = fx.cton_type(substs.type_at(0)).unwrap();
            let ptr = args[0].load_value(fx);
            let amount = args[1].load_value(fx);
            let old = fx.bcx.ins().load(clif_ty, MemFlags::new(), ptr, 0);
            let new = fx.bcx.ins().iadd(old, amount);
            fx.bcx.ins().store(MemFlags::new(), new, ptr, 0);
            ret.write_cvalue(fx, CValue::ByVal(old, fx.layout_of(substs.type_at(0))));
        }
        _ if intrinsic.starts_with("atomic_xsub") => {
            assert_eq!(args.len(), 2);
            let clif_ty = fx.cton_type(substs.type_at(0)).unwrap();
            let ptr = args[0].load_value(fx);
            let amount = args[1].load_value(fx);
            let old = fx.bcx.ins().load(clif_ty, MemFlags::new(), ptr, 0);
            let new = fx.bcx.ins().isub(old, amount);
            fx.bcx.ins().store(MemFlags::new(), new, ptr, 0);
            ret.write_cvalue(fx, CValue::ByVal(old, fx.layout_of(substs.type_at(0))));
        }
        _ => unimpl!("unsupported intrinsic {}", intrinsic),
    }

    if let Some((_, dest)) = destination {
        let ret_ebb = fx.get_ebb(dest);
        fx.bcx.ins().jump(ret_ebb, &[]);
    } else {
        fx.bcx.ins().trap(TrapCode::User(!0));
    }
}
