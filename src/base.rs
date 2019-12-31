use rustc::ty::adjustment::PointerCast;

use crate::prelude::*;

pub fn trans_fn<'clif, 'tcx, B: Backend + 'static>(
    cx: &mut crate::CodegenCx<'clif, 'tcx, B>,
    instance: Instance<'tcx>,
    linkage: Linkage,
) {
    let tcx = cx.tcx;

    let mir = *tcx.instance_mir(instance.def);

    // Declare function
    let (name, sig) = get_function_name_and_sig(tcx, cx.module.isa().triple(), instance, false);
    let func_id = cx.module.declare_function(&name, linkage, &sig).unwrap();
    let mut debug_context = cx
        .debug_context
        .as_mut()
        .map(|debug_context| FunctionDebugContext::new(debug_context, instance, func_id, &name));

    // Make FunctionBuilder
    let mut func = Function::with_name_signature(ExternalName::user(0, 0), sig);
    func.collect_debug_info();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut bcx = FunctionBuilder::new(&mut func, &mut func_ctx);

    // Predefine ebb's
    let start_ebb = bcx.create_ebb();
    let mut ebb_map: HashMap<BasicBlock, Ebb> = HashMap::new();
    for (bb, _bb_data) in mir.basic_blocks().iter_enumerated() {
        ebb_map.insert(bb, bcx.create_ebb());
    }

    // Make FunctionCx
    let pointer_type = cx.module.target_config().pointer_type();
    let clif_comments = crate::pretty_clif::CommentWriter::new(tcx, instance);

    let mut fx = FunctionCx {
        tcx,
        module: cx.module,
        pointer_type,

        instance,
        mir,

        bcx,
        ebb_map,
        local_map: HashMap::new(),

        clif_comments,
        constants_cx: &mut cx.constants_cx,
        caches: &mut cx.caches,
        source_info_set: indexmap::IndexSet::new(),
    };

    if fx.mir.args_iter().any(|arg| fx.layout_of(fx.monomorphize(&fx.mir.local_decls[arg].ty)).abi.is_uninhabited()) {
        let entry_block = fx.bcx.create_ebb();
        fx.bcx.append_ebb_params_for_function_params(entry_block);
        fx.bcx.switch_to_block(entry_block);
        crate::trap::trap_unreachable(&mut fx, "function has uninhabited argument");
    } else {
        crate::abi::codegen_fn_prelude(&mut fx, start_ebb);
        codegen_fn_content(&mut fx);
    }

    // Recover all necessary data from fx, before accessing func will prevent future access to it.
    let instance = fx.instance;
    let mut clif_comments = fx.clif_comments;
    let source_info_set = fx.source_info_set;
    let local_map = fx.local_map;

    #[cfg(debug_assertions)]
    crate::pretty_clif::write_clif_file(cx.tcx, "unopt", instance, &func, &clif_comments, None);

    // Verify function
    verify_func(tcx, &clif_comments, &func);

    let context = &mut cx.caches.context;
    context.func = func;

    // Perform rust specific optimizations
    crate::optimize::optimize_function(cx.tcx, instance, context, &mut clif_comments);

    // Define function
    cx.module.define_function(func_id, context).unwrap();

    // Write optimized function to file for debugging
    #[cfg(debug_assertions)]
    {
        let value_ranges = context
            .build_value_labels_ranges(cx.module.isa())
            .expect("value location ranges");

        crate::pretty_clif::write_clif_file(
            cx.tcx,
            "opt",
            instance,
            &context.func,
            &clif_comments,
            Some(&value_ranges),
        );
    }

    // Define debuginfo for function
    let isa = cx.module.isa();
    debug_context
        .as_mut()
        .map(|x| x.define(context, isa, &source_info_set, local_map));

    // Clear context to make it usable for the next function
    context.clear();
}

pub fn verify_func(tcx: TyCtxt, writer: &crate::pretty_clif::CommentWriter, func: &Function) {
    let flags = settings::Flags::new(settings::builder());
    match ::cranelift_codegen::verify_function(&func, &flags) {
        Ok(_) => {}
        Err(err) => {
            tcx.sess.err(&format!("{:?}", err));
            let pretty_error = ::cranelift_codegen::print_errors::pretty_verifier_error(
                &func,
                None,
                Some(Box::new(writer)),
                err,
            );
            tcx.sess
                .fatal(&format!("cranelift verify error:\n{}", pretty_error));
        }
    }
}

fn codegen_fn_content(fx: &mut FunctionCx<'_, '_, impl Backend>) {
    for (bb, bb_data) in fx.mir.basic_blocks().iter_enumerated() {
        if bb_data.is_cleanup {
            // Unwinding after panicking is not supported
            continue;
        }

        let ebb = fx.get_ebb(bb);
        fx.bcx.switch_to_block(ebb);

        fx.bcx.ins().nop();
        for stmt in &bb_data.statements {
            fx.set_debug_loc(stmt.source_info);
            trans_stmt(fx, ebb, stmt);
        }

        #[cfg(debug_assertions)]
        {
            let mut terminator_head = "\n".to_string();
            bb_data
                .terminator()
                .kind
                .fmt_head(&mut terminator_head)
                .unwrap();
            let inst = fx.bcx.func.layout.last_inst(ebb).unwrap();
            fx.add_comment(inst, terminator_head);
        }

        fx.set_debug_loc(bb_data.terminator().source_info);

        match &bb_data.terminator().kind {
            TerminatorKind::Goto { target } => {
                let ebb = fx.get_ebb(*target);
                fx.bcx.ins().jump(ebb, &[]);
            }
            TerminatorKind::Return => {
                crate::abi::codegen_return(fx);
            }
            TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                cleanup: _,
            } => {
                if !fx.tcx.sess.overflow_checks() {
                    if let mir::interpret::PanicInfo::OverflowNeg = *msg {
                        let target = fx.get_ebb(*target);
                        fx.bcx.ins().jump(target, &[]);
                        continue;
                    }
                }
                let cond = trans_operand(fx, cond).load_scalar(fx);
                let target = fx.get_ebb(*target);
                if *expected {
                    fx.bcx.ins().brnz(cond, target, &[]);
                } else {
                    fx.bcx.ins().brz(cond, target, &[]);
                };
                trap_panic(
                    fx,
                    format!(
                        "[panic] Assert {:?} at {:?} failed.",
                        msg,
                        bb_data.terminator().source_info.span
                    ),
                );
            }

            TerminatorKind::SwitchInt {
                discr,
                switch_ty: _,
                values,
                targets,
            } => {
                let discr = trans_operand(fx, discr).load_scalar(fx);
                let mut switch = ::cranelift_frontend::Switch::new();
                for (i, value) in values.iter().enumerate() {
                    let ebb = fx.get_ebb(targets[i]);
                    switch.set_entry(*value as u64, ebb);
                }
                let otherwise_ebb = fx.get_ebb(targets[targets.len() - 1]);
                switch.emit(&mut fx.bcx, discr, otherwise_ebb);
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                cleanup: _,
                from_hir_call: _,
            } => {
                crate::abi::codegen_terminator_call(
                    fx,
                    func,
                    args,
                    destination,
                    bb_data.terminator().source_info.span,
                );
            }
            TerminatorKind::Resume | TerminatorKind::Abort => {
                trap_unreachable(fx, "[corruption] Unwinding bb reached.");
            }
            TerminatorKind::Unreachable => {
                trap_unreachable(fx, "[corruption] Hit unreachable code.");
            }
            TerminatorKind::Yield { .. }
            | TerminatorKind::FalseEdges { .. }
            | TerminatorKind::FalseUnwind { .. }
            | TerminatorKind::DropAndReplace { .. }
            | TerminatorKind::GeneratorDrop => {
                bug!("shouldn't exist at trans {:?}", bb_data.terminator());
            }
            TerminatorKind::Drop {
                location,
                target,
                unwind: _,
            } => {
                let drop_place = trans_place(fx, location);
                crate::abi::codegen_drop(fx, drop_place);

                let target_ebb = fx.get_ebb(*target);
                fx.bcx.ins().jump(target_ebb, &[]);
            }
        };
    }

    fx.bcx.seal_all_blocks();
    fx.bcx.finalize();
}

fn trans_stmt<'tcx>(
    fx: &mut FunctionCx<'_, 'tcx, impl Backend>,
    cur_ebb: Ebb,
    stmt: &Statement<'tcx>,
) {
    let _print_guard = PrintOnPanic(|| format!("stmt {:?}", stmt));

    fx.set_debug_loc(stmt.source_info);

    #[cfg(false_debug_assertions)]
    match &stmt.kind {
        StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => {} // Those are not very useful
        _ => {
            let inst = fx.bcx.func.layout.last_inst(cur_ebb).unwrap();
            fx.add_comment(inst, format!("{:?}", stmt));
        }
    }

    match &stmt.kind {
        StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => {
            let place = trans_place(fx, place);
            crate::discriminant::codegen_set_discriminant(fx, place, *variant_index);
        }
        StatementKind::Assign(to_place_and_rval) => {
            let lval = trans_place(fx, &to_place_and_rval.0);
            let dest_layout = lval.layout();
            match &to_place_and_rval.1 {
                Rvalue::Use(operand) => {
                    let val = trans_operand(fx, operand);
                    lval.write_cvalue(fx, val);
                }
                Rvalue::Ref(_, _, place) | Rvalue::AddressOf(_, place) => {
                    let place = trans_place(fx, place);
                    place.write_place_ref(fx, lval);
                }
                Rvalue::BinaryOp(bin_op, lhs, rhs) => {
                    let lhs = trans_operand(fx, lhs);
                    let rhs = trans_operand(fx, rhs);

                    let res = crate::num::codegen_binop(fx, *bin_op, lhs, rhs);
                    lval.write_cvalue(fx, res);
                }
                Rvalue::CheckedBinaryOp(bin_op, lhs, rhs) => {
                    let lhs = trans_operand(fx, lhs);
                    let rhs = trans_operand(fx, rhs);

                    let res = if !fx.tcx.sess.overflow_checks() {
                        let val =
                            crate::num::trans_int_binop(fx, *bin_op, lhs, rhs).load_scalar(fx);
                        let is_overflow = fx.bcx.ins().iconst(types::I8, 0);
                        CValue::by_val_pair(val, is_overflow, lval.layout())
                    } else {
                        crate::num::trans_checked_int_binop(fx, *bin_op, lhs, rhs)
                    };

                    lval.write_cvalue(fx, res);
                }
                Rvalue::UnaryOp(un_op, operand) => {
                    let operand = trans_operand(fx, operand);
                    let layout = operand.layout();
                    let val = operand.load_scalar(fx);
                    let res = match un_op {
                        UnOp::Not => {
                            match layout.ty.kind {
                                ty::Bool => {
                                    let val = fx.bcx.ins().uextend(types::I32, val); // WORKAROUND for CraneStation/cranelift#466
                                    let res = fx.bcx.ins().icmp_imm(IntCC::Equal, val, 0);
                                    CValue::by_val(fx.bcx.ins().bint(types::I8, res), layout)
                                }
                                ty::Uint(_) | ty::Int(_) => {
                                    CValue::by_val(fx.bcx.ins().bnot(val), layout)
                                }
                                _ => unimplemented!("un op Not for {:?}", layout.ty),
                            }
                        }
                        UnOp::Neg => match layout.ty.kind {
                            ty::Int(_) => {
                                let zero = CValue::const_val(fx, layout.ty, 0);
                                crate::num::trans_int_binop(fx, BinOp::Sub, zero, operand)
                            }
                            ty::Float(_) => {
                                CValue::by_val(fx.bcx.ins().fneg(val), layout)
                            }
                            _ => unimplemented!("un op Neg for {:?}", layout.ty),
                        },
                    };
                    lval.write_cvalue(fx, res);
                }
                Rvalue::Cast(CastKind::Pointer(PointerCast::ReifyFnPointer), operand, to_ty) => {
                    let from_ty = fx.monomorphize(&operand.ty(&fx.mir.local_decls, fx.tcx));
                    let to_layout = fx.layout_of(fx.monomorphize(to_ty));
                    match from_ty.kind {
                        ty::FnDef(def_id, substs) => {
                            let func_ref = fx.get_function_ref(
                                Instance::resolve(fx.tcx, ParamEnv::reveal_all(), def_id, substs)
                                    .unwrap(),
                            );
                            let func_addr = fx.bcx.ins().func_addr(fx.pointer_type, func_ref);
                            lval.write_cvalue(fx, CValue::by_val(func_addr, to_layout));
                        }
                        _ => bug!("Trying to ReifyFnPointer on non FnDef {:?}", from_ty),
                    }
                }
                Rvalue::Cast(CastKind::Pointer(PointerCast::UnsafeFnPointer), operand, to_ty)
                | Rvalue::Cast(CastKind::Pointer(PointerCast::MutToConstPointer), operand, to_ty)
                | Rvalue::Cast(CastKind::Pointer(PointerCast::ArrayToPointer), operand, to_ty) => {
                    let to_layout = fx.layout_of(fx.monomorphize(to_ty));
                    let operand = trans_operand(fx, operand);
                    lval.write_cvalue(fx, operand.unchecked_cast_to(to_layout));
                }
                Rvalue::Cast(CastKind::Misc, operand, to_ty) => {
                    let operand = trans_operand(fx, operand);
                    let from_ty = operand.layout().ty;
                    let to_ty = fx.monomorphize(to_ty);

                    fn is_fat_ptr<'tcx>(
                        fx: &FunctionCx<'_, 'tcx, impl Backend>,
                        ty: Ty<'tcx>,
                    ) -> bool {
                        ty.builtin_deref(true)
                            .map(
                                |ty::TypeAndMut {
                                     ty: pointee_ty,
                                     mutbl: _,
                                 }| has_ptr_meta(fx.tcx, pointee_ty),
                            )
                            .unwrap_or(false)
                    }

                    if is_fat_ptr(fx, from_ty) {
                        if is_fat_ptr(fx, to_ty) {
                            // fat-ptr -> fat-ptr
                            lval.write_cvalue(fx, operand.unchecked_cast_to(dest_layout));
                        } else {
                            // fat-ptr -> thin-ptr
                            let (ptr, _extra) = operand.load_scalar_pair(fx);
                            lval.write_cvalue(fx, CValue::by_val(ptr, dest_layout))
                        }
                    } else if let ty::Adt(adt_def, _substs) = from_ty.kind {
                        // enum -> discriminant value
                        assert!(adt_def.is_enum());
                        match to_ty.kind {
                            ty::Uint(_) | ty::Int(_) => {}
                            _ => unreachable!("cast adt {} -> {}", from_ty, to_ty),
                        }

                        let discr = crate::discriminant::codegen_get_discriminant(
                            fx,
                            operand,
                            fx.layout_of(to_ty),
                        );
                        lval.write_cvalue(fx, discr);
                    } else {
                        let to_clif_ty = fx.clif_type(to_ty).unwrap();
                        let from = operand.load_scalar(fx);

                        let res = clif_int_or_float_cast(
                            fx,
                            from,
                            type_sign(from_ty),
                            to_clif_ty,
                            type_sign(to_ty),
                        );
                        lval.write_cvalue(fx, CValue::by_val(res, dest_layout));
                    }
                }
                Rvalue::Cast(CastKind::Pointer(PointerCast::ClosureFnPointer(_)), operand, _to_ty) => {
                    let operand = trans_operand(fx, operand);
                    match operand.layout().ty.kind {
                        ty::Closure(def_id, substs) => {
                            let instance = Instance::resolve_closure(
                                fx.tcx,
                                def_id,
                                substs,
                                ty::ClosureKind::FnOnce,
                            );
                            let func_ref = fx.get_function_ref(instance);
                            let func_addr = fx.bcx.ins().func_addr(fx.pointer_type, func_ref);
                            lval.write_cvalue(fx, CValue::by_val(func_addr, lval.layout()));
                        }
                        _ => bug!("{} cannot be cast to a fn ptr", operand.layout().ty),
                    }
                }
                Rvalue::Cast(CastKind::Pointer(PointerCast::Unsize), operand, _to_ty) => {
                    let operand = trans_operand(fx, operand);
                    operand.unsize_value(fx, lval);
                }
                Rvalue::Discriminant(place) => {
                    let place = trans_place(fx, place);
                    let value = place.to_cvalue(fx);
                    let discr =
                        crate::discriminant::codegen_get_discriminant(fx, value, dest_layout);
                    lval.write_cvalue(fx, discr);
                }
                Rvalue::Repeat(operand, times) => {
                    let operand = trans_operand(fx, operand);
                    for i in 0..*times {
                        let index = fx.bcx.ins().iconst(fx.pointer_type, i as i64);
                        let to = lval.place_index(fx, index);
                        to.write_cvalue(fx, operand);
                    }
                }
                Rvalue::Len(place) => {
                    let place = trans_place(fx, place);
                    let usize_layout = fx.layout_of(fx.tcx.types.usize);
                    let len = codegen_array_len(fx, place);
                    lval.write_cvalue(fx, CValue::by_val(len, usize_layout));
                }
                Rvalue::NullaryOp(NullOp::Box, content_ty) => {
                    use rustc::middle::lang_items::ExchangeMallocFnLangItem;

                    let usize_type = fx.clif_type(fx.tcx.types.usize).unwrap();
                    let content_ty = fx.monomorphize(content_ty);
                    let layout = fx.layout_of(content_ty);
                    let llsize = fx.bcx.ins().iconst(usize_type, layout.size.bytes() as i64);
                    let llalign = fx
                        .bcx
                        .ins()
                        .iconst(usize_type, layout.align.abi.bytes() as i64);
                    let box_layout = fx.layout_of(fx.tcx.mk_box(content_ty));

                    // Allocate space:
                    let def_id = match fx.tcx.lang_items().require(ExchangeMallocFnLangItem) {
                        Ok(id) => id,
                        Err(s) => {
                            fx.tcx
                                .sess
                                .fatal(&format!("allocation of `{}` {}", box_layout.ty, s));
                        }
                    };
                    let instance = ty::Instance::mono(fx.tcx, def_id);
                    let func_ref = fx.get_function_ref(instance);
                    let call = fx.bcx.ins().call(func_ref, &[llsize, llalign]);
                    let ptr = fx.bcx.inst_results(call)[0];
                    lval.write_cvalue(fx, CValue::by_val(ptr, box_layout));
                }
                Rvalue::NullaryOp(NullOp::SizeOf, ty) => {
                    assert!(lval
                        .layout()
                        .ty
                        .is_sized(fx.tcx.at(DUMMY_SP), ParamEnv::reveal_all()));
                    let ty_size = fx.layout_of(fx.monomorphize(ty)).size.bytes();
                    let val = CValue::const_val(fx, fx.tcx.types.usize, ty_size.into());
                    lval.write_cvalue(fx, val);
                }
                Rvalue::Aggregate(kind, operands) => match **kind {
                    AggregateKind::Array(_ty) => {
                        for (i, operand) in operands.into_iter().enumerate() {
                            let operand = trans_operand(fx, operand);
                            let index = fx.bcx.ins().iconst(fx.pointer_type, i as i64);
                            let to = lval.place_index(fx, index);
                            to.write_cvalue(fx, operand);
                        }
                    }
                    _ => unreachable!("shouldn't exist at trans {:?}", to_place_and_rval.1),
                },
            }
        }
        StatementKind::StorageLive(_)
        | StatementKind::StorageDead(_)
        | StatementKind::Nop
        | StatementKind::FakeRead(..)
        | StatementKind::Retag { .. }
        | StatementKind::AscribeUserType(..) => {}

        StatementKind::InlineAsm(asm) => {
            use syntax::ast::Name;
            let InlineAsm {
                asm,
                outputs: _,
                inputs: _,
            } = &**asm;
            let rustc::hir::InlineAsmInner {
                asm: asm_code, // Name
                outputs,       // Vec<Name>
                inputs,        // Vec<Name>
                clobbers,      // Vec<Name>
                volatile,      // bool
                alignstack,    // bool
                dialect: _,    // syntax::ast::AsmDialect
                asm_str_style: _,
            } = asm;
            match &*asm_code.as_str() {
                "" => {
                    assert_eq!(inputs, &[Name::intern("r")]);
                    assert!(outputs.is_empty(), "{:?}", outputs);

                    // Black box
                }
                "cpuid" | "cpuid\n" => {
                    assert_eq!(inputs, &[Name::intern("{eax}"), Name::intern("{ecx}")]);

                    assert_eq!(outputs.len(), 4);
                    for (i, c) in (&["={eax}", "={ebx}", "={ecx}", "={edx}"])
                        .iter()
                        .enumerate()
                    {
                        assert_eq!(&outputs[i].constraint.as_str(), c);
                        assert!(!outputs[i].is_rw);
                        assert!(!outputs[i].is_indirect);
                    }

                    assert_eq!(clobbers, &[Name::intern("rbx")]);

                    assert!(!volatile);
                    assert!(!alignstack);

                    crate::trap::trap_unimplemented(
                        fx,
                        "__cpuid_count arch intrinsic is not supported",
                    );
                }
                "xgetbv" => {
                    assert_eq!(inputs, &[Name::intern("{ecx}")]);

                    assert_eq!(outputs.len(), 2);
                    for (i, c) in (&["={eax}", "={edx}"]).iter().enumerate() {
                        assert_eq!(&outputs[i].constraint.as_str(), c);
                        assert!(!outputs[i].is_rw);
                        assert!(!outputs[i].is_indirect);
                    }

                    assert_eq!(clobbers, &[]);

                    assert!(!volatile);
                    assert!(!alignstack);

                    crate::trap::trap_unimplemented(fx, "_xgetbv arch intrinsic is not supported");
                }
                _ if fx.tcx.symbol_name(fx.instance).name.as_str() == "__rust_probestack" => {
                    crate::trap::trap_unimplemented(fx, "__rust_probestack is not supported");
                }
                _ => unimpl!("Inline assembly is not supported"),
            }
        }
    }
}

fn codegen_array_len<'tcx>(
    fx: &mut FunctionCx<'_, 'tcx, impl Backend>,
    place: CPlace<'tcx>,
) -> Value {
    match place.layout().ty.kind {
        ty::Array(_elem_ty, len) => {
            let len = crate::constant::force_eval_const(fx, len)
                .eval_usize(fx.tcx, ParamEnv::reveal_all()) as i64;
            fx.bcx.ins().iconst(fx.pointer_type, len)
        }
        ty::Slice(_elem_ty) => place
            .to_ptr_maybe_unsized(fx)
            .1
            .expect("Length metadata for slice place"),
        _ => bug!("Rvalue::Len({:?})", place),
    }
}

pub fn trans_place<'tcx>(
    fx: &mut FunctionCx<'_, 'tcx, impl Backend>,
    place: &Place<'tcx>,
) -> CPlace<'tcx> {
    let mut cplace = match &place.base {
        PlaceBase::Local(local) => fx.get_local_place(*local),
        PlaceBase::Static(static_) => match static_.kind {
            StaticKind::Static => {
                // Statics can't be generic, so `static_.ty` doesn't need to be monomorphized.
                crate::constant::codegen_static_ref(fx, static_.def_id, static_.ty)
            }
            StaticKind::Promoted(promoted, substs) => {
                let instance = Instance::new(static_.def_id, fx.monomorphize(&substs));
                let ty = fx.monomorphize(&static_.ty);
                crate::constant::trans_promoted(fx, instance, promoted, ty)
            }
        },
    };

    for elem in &*place.projection {
        match *elem {
            PlaceElem::Deref => {
                cplace = cplace.place_deref(fx);
            }
            PlaceElem::Field(field, _ty) => {
                cplace = cplace.place_field(fx, field);
            }
            PlaceElem::Index(local) => {
                let index = fx.get_local_place(local).to_cvalue(fx).load_scalar(fx);
                cplace = cplace.place_index(fx, index);
            }
            PlaceElem::ConstantIndex {
                offset,
                min_length: _,
                from_end,
            } => {
                let index = if !from_end {
                    fx.bcx.ins().iconst(fx.pointer_type, offset as i64)
                } else {
                    let len = codegen_array_len(fx, cplace);
                    fx.bcx.ins().iadd_imm(len, -(offset as i64))
                };
                cplace = cplace.place_index(fx, index);
            }
            PlaceElem::Subslice { from, to, from_end } => {
                // These indices are generated by slice patterns.
                // slice[from:-to] in Python terms.

                match cplace.layout().ty.kind {
                    ty::Array(elem_ty, len) => {
                        let elem_layout = fx.layout_of(elem_ty);
                        let ptr = cplace.to_ptr(fx);
                        let len = crate::constant::force_eval_const(fx, len)
                            .eval_usize(fx.tcx, ParamEnv::reveal_all());
                        cplace = CPlace::for_ptr(
                            ptr.offset_i64(fx, elem_layout.size.bytes() as i64 * from as i64),
                            fx.layout_of(fx.tcx.mk_array(elem_ty, len - from as u64 - to as u64)),
                        );
                    }
                    ty::Slice(elem_ty) => {
                        assert!(from_end, "slice subslices should be `from_end`");
                        let elem_layout = fx.layout_of(elem_ty);
                        let (ptr, len) = cplace.to_ptr_maybe_unsized(fx);
                        let len = len.unwrap();
                        cplace = CPlace::for_ptr_with_extra(
                            ptr.offset_i64(fx, elem_layout.size.bytes() as i64 * from as i64),
                            fx.bcx.ins().iadd_imm(len, -(from as i64 + to as i64)),
                            cplace.layout(),
                        );
                    }
                    _ => unreachable!(),
                }
            }
            PlaceElem::Downcast(_adt_def, variant) => {
                cplace = cplace.downcast_variant(fx, variant);
            }
        }
    }

    cplace
}

pub fn trans_operand<'tcx>(
    fx: &mut FunctionCx<'_, 'tcx, impl Backend>,
    operand: &Operand<'tcx>,
) -> CValue<'tcx> {
    match operand {
        Operand::Move(place) | Operand::Copy(place) => {
            let cplace = trans_place(fx, place);
            cplace.to_cvalue(fx)
        }
        Operand::Constant(const_) => crate::constant::trans_constant(fx, const_),
    }
}
