use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::traits::{self, Reveal};
use rustc::ty::fold::TypeFoldable;
use rustc::ty::layout::{Layout, Size};
use rustc::ty::subst::Substs;
use rustc::ty::{self, Ty, TyCtxt, BareFnTy};
use syntax::codemap::{DUMMY_SP, Span};
use syntax::{ast, attr, abi};

use error::{EvalError, EvalResult};
use eval_context::{EvalContext, IntegerExt, StackPopCleanup, is_inhabited};
use lvalue::Lvalue;
use memory::{Pointer, FunctionDefinition, Function};
use value::PrimVal;
use value::Value;

mod intrinsic;
mod drop;

impl<'a, 'tcx> EvalContext<'a, 'tcx> {

    pub(super) fn goto_block(&mut self, target: mir::BasicBlock) {
        self.frame_mut().block = target;
        self.frame_mut().stmt = 0;
    }

    pub(super) fn eval_terminator(
        &mut self,
        terminator: &mir::Terminator<'tcx>,
    ) -> EvalResult<'tcx> {
        use rustc::mir::TerminatorKind::*;
        match terminator.kind {
            Return => {
                self.dump_local(self.frame().return_lvalue);
                self.pop_stack_frame()?
            }

            Goto { target } => self.goto_block(target),

            If { ref cond, targets: (then_target, else_target) } => {
                let cond_val = self.eval_operand_to_primval(cond)?.to_bool()?;
                self.goto_block(if cond_val { then_target } else { else_target });
            }

            SwitchInt { ref discr, ref values, ref targets, .. } => {
                let discr_val = self.eval_and_read_lvalue(discr)?;
                let discr_ty = self.lvalue_ty(discr);
                let discr_prim = self.value_to_primval(discr_val, discr_ty)?;

                // Branch to the `otherwise` case by default, if no match is found.
                let mut target_block = targets[targets.len() - 1];

                for (index, const_val) in values.iter().enumerate() {
                    let val = self.const_to_value(const_val)?;
                    let prim = self.value_to_primval(val, discr_ty)?;
                    if discr_prim.to_bytes()? == prim.to_bytes()? {
                        target_block = targets[index];
                        break;
                    }
                }

                self.goto_block(target_block);
            }

            Switch { ref discr, ref targets, adt_def } => {
                // FIXME(solson)
                let lvalue = self.eval_lvalue(discr)?;
                let lvalue = self.force_allocation(lvalue)?;

                let adt_ptr = lvalue.to_ptr();
                let adt_ty = self.lvalue_ty(discr);
                let discr_val = self.read_discriminant_value(adt_ptr, adt_ty)?;
                let matching = adt_def.variants.iter()
                    .position(|v| discr_val == v.disr_val.to_u128_unchecked());

                match matching {
                    Some(i) => self.goto_block(targets[i]),
                    None => return Err(EvalError::InvalidDiscriminant),
                }
            }

            Call { ref func, ref args, ref destination, .. } => {
                let destination = match *destination {
                    Some((ref lv, target)) => Some((self.eval_lvalue(lv)?, target)),
                    None => None,
                };

                let func_ty = self.operand_ty(func);
                match func_ty.sty {
                    ty::TyFnPtr(bare_fn_ty) => {
                        let fn_ptr = self.eval_operand_to_primval(func)?.to_ptr()?;
                        let FunctionDefinition {def_id, substs, abi, sig} = self.memory.get_fn(fn_ptr.alloc_id)?.expect_concrete()?;
                        let bare_sig = self.tcx.erase_late_bound_regions_and_normalize(&bare_fn_ty.sig);
                        let bare_sig = self.tcx.erase_regions(&bare_sig);
                        // transmuting function pointers in miri is fine as long as the number of
                        // arguments and the abi don't change.
                        // FIXME: also check the size of the arguments' type and the return type
                        // Didn't get it to work, since that triggers an assertion in rustc which
                        // checks whether the type has escaping regions
                        if abi != bare_fn_ty.abi ||
                           sig.variadic != bare_sig.variadic ||
                           sig.inputs().len() != bare_sig.inputs().len() {
                            return Err(EvalError::FunctionPointerTyMismatch(abi, sig, bare_fn_ty));
                        }
                        self.eval_fn_call(def_id, substs, bare_fn_ty, destination, args,
                                          terminator.source_info.span)?
                    },
                    ty::TyFnDef(def_id, substs, fn_ty) => {
                        self.eval_fn_call(def_id, substs, fn_ty, destination, args,
                                          terminator.source_info.span)?
                    }

                    _ => {
                        let msg = format!("can't handle callee of type {:?}", func_ty);
                        return Err(EvalError::Unimplemented(msg));
                    }
                }
            }

            Drop { ref location, target, .. } => {
                let lval = self.eval_lvalue(location)?;

                let ty = self.lvalue_ty(location);

                // we can't generate the drop stack frames on the fly,
                // because that would change our call stack
                // and very much confuse the further processing of the drop glue
                let mut drops = Vec::new();
                self.drop(lval, ty, &mut drops)?;
                self.goto_block(target);
                self.eval_drop_impls(drops, terminator.source_info.span)?;
            }

            Assert { ref cond, expected, ref msg, target, .. } => {
                let cond_val = self.eval_operand_to_primval(cond)?.to_bool()?;
                if expected == cond_val {
                    self.goto_block(target);
                } else {
                    return match *msg {
                        mir::AssertMessage::BoundsCheck { ref len, ref index } => {
                            let span = terminator.source_info.span;
                            let len = self.eval_operand_to_primval(len)
                                .expect("can't eval len")
                                .to_u64()?;
                            let index = self.eval_operand_to_primval(index)
                                .expect("can't eval index")
                                .to_u64()?;
                            Err(EvalError::ArrayIndexOutOfBounds(span, len, index))
                        },
                        mir::AssertMessage::Math(ref err) =>
                            Err(EvalError::Math(terminator.source_info.span, err.clone())),
                    }
                }
            },

            DropAndReplace { .. } => unimplemented!(),
            Resume => unimplemented!(),
            Unreachable => return Err(EvalError::Unreachable),
        }

        Ok(())
    }

    fn eval_fn_call(
        &mut self,
        def_id: DefId,
        substs: &'tcx Substs<'tcx>,
        fn_ty: &'tcx BareFnTy,
        destination: Option<(Lvalue<'tcx>, mir::BasicBlock)>,
        arg_operands: &[mir::Operand<'tcx>],
        span: Span,
    ) -> EvalResult<'tcx> {
        use syntax::abi::Abi;
        match fn_ty.abi {
            Abi::RustIntrinsic => {
                let ty = fn_ty.sig.0.output();
                let layout = self.type_layout(ty)?;
                let (ret, target) = match destination {
                    Some(dest) if is_inhabited(self.tcx, ty) => dest,
                    _ => return Err(EvalError::Unreachable),
                };
                self.call_intrinsic(def_id, substs, arg_operands, ret, ty, layout, target)?;
                self.dump_local(ret);
                Ok(())
            }

            Abi::C => {
                let ty = fn_ty.sig.0.output();
                let (ret, target) = destination.unwrap();
                self.call_c_abi(def_id, arg_operands, ret, ty)?;
                self.dump_local(ret);
                self.goto_block(target);
                Ok(())
            }

            Abi::Rust | Abi::RustCall => {
                let mut args = Vec::new();
                for arg in arg_operands {
                    let arg_val = self.eval_operand(arg)?;
                    let arg_ty = self.operand_ty(arg);
                    args.push((arg_val, arg_ty));
                }

                // Only trait methods can have a Self parameter.
                let (resolved_def_id, resolved_substs, temporaries) =
                    if let Some(trait_id) = self.tcx.trait_of_item(def_id) {
                        self.trait_method(trait_id, def_id, substs, &mut args)?
                    } else {
                        (def_id, substs, Vec::new())
                    };

                // FIXME(eddyb) Detect ADT constructors more efficiently.
                if let Some(adt_def) = fn_ty.sig.skip_binder().output().ty_adt_def() {
                    if let Some(v) = adt_def.variants.iter().find(|v| resolved_def_id == v.did) {
                        let (lvalue, target) = destination.expect("tuple struct constructors can't diverge");
                        let dest_ty = self.tcx.item_type(adt_def.did);
                        let dest_layout = self.type_layout(dest_ty)?;
                        trace!("layout({:?}) = {:#?}", dest_ty, dest_layout);
                        match *dest_layout {
                            Layout::Univariant { ref variant, .. } => {
                                let disr_val = v.disr_val.to_u128_unchecked();
                                assert_eq!(disr_val, 0);
                                let offsets = variant.offsets.iter().map(|s| s.bytes());

                                self.assign_fields(lvalue, offsets, args)?;
                            },
                            Layout::General { discr, ref variants, .. } => {
                                let disr_val = v.disr_val.to_u128_unchecked();
                                let discr_size = discr.size().bytes();
                                self.assign_discr_and_fields(
                                    lvalue,
                                    variants[disr_val as usize].offsets.iter().cloned().map(Size::bytes),
                                    args,
                                    disr_val,
                                    discr_size,
                                )?;
                            },
                            Layout::StructWrappedNullablePointer { nndiscr, ref nonnull, ref discrfield, .. } => {
                                let disr_val = v.disr_val.to_u128_unchecked();
                                if nndiscr as u128 == disr_val {
                                    let offsets = nonnull.offsets.iter().map(|s| s.bytes());
                                    self.assign_fields(lvalue, offsets, args)?;
                                } else {
                                    for (_, ty) in args {
                                        assert_eq!(self.type_size(ty)?, Some(0));
                                    }
                                    let (offset, ty) = self.nonnull_offset_and_ty(dest_ty, nndiscr, discrfield)?;

                                    // FIXME(solson)
                                    let dest = self.force_allocation(lvalue)?.to_ptr();

                                    let dest = dest.offset(offset.bytes());
                                    let dest_size = self.type_size(ty)?
                                        .expect("bad StructWrappedNullablePointer discrfield");
                                    self.memory.write_int(dest, 0, dest_size)?;
                                }
                            },
                            Layout::RawNullablePointer { .. } => {
                                assert_eq!(args.len(), 1);
                                let (val, ty) = args.pop().unwrap();
                                self.write_value(val, lvalue, ty)?;
                            },
                            _ => bug!("bad layout for tuple struct constructor: {:?}", dest_layout),
                        }
                        self.goto_block(target);
                        return Ok(());
                    }
                }

                let mir = match self.load_mir(resolved_def_id) {
                    Ok(mir) => mir,
                    Err(EvalError::NoMirFor(path)) => {
                        match &path[..] {
                            // let's just ignore all output for now
                            "std::io::_print" => {
                                self.goto_block(destination.unwrap().1);
                                return Ok(());
                            },
                            "std::thread::Builder::new" => return Err(EvalError::Unimplemented("miri does not support threading".to_owned())),
                            "std::env::args" => return Err(EvalError::Unimplemented("miri does not support program arguments".to_owned())),
                            "std::panicking::rust_panic_with_hook" |
                            "std::rt::begin_panic_fmt" => return Err(EvalError::Panic),
                            "std::panicking::panicking" |
                            "std::rt::panicking" => {
                                let (lval, block) = destination.expect("std::rt::panicking does not diverge");
                                // we abort on panic -> `std::rt::panicking` always returns false
                                let bool = self.tcx.types.bool;
                                self.write_primval(lval, PrimVal::from_bool(false), bool)?;
                                self.goto_block(block);
                                return Ok(());
                            }
                            _ => {},
                        }
                        return Err(EvalError::NoMirFor(path));
                    },
                    Err(other) => return Err(other),
                };
                let (return_lvalue, return_to_block) = match destination {
                    Some((lvalue, block)) => (lvalue, StackPopCleanup::Goto(block)),
                    None => {
                        // FIXME(solson)
                        let lvalue = Lvalue::from_ptr(Pointer::never_ptr());
                        (lvalue, StackPopCleanup::None)
                    }
                };

                self.push_stack_frame(
                    resolved_def_id,
                    span,
                    mir,
                    resolved_substs,
                    return_lvalue,
                    return_to_block,
                    temporaries,
                )?;

                let arg_locals = self.frame().mir.args_iter();
                assert_eq!(self.frame().mir.arg_count, args.len());
                for (arg_local, (arg_val, arg_ty)) in arg_locals.zip(args) {
                    let dest = self.eval_lvalue(&mir::Lvalue::Local(arg_local))?;
                    self.write_value(arg_val, dest, arg_ty)?;
                }

                Ok(())
            }

            abi => Err(EvalError::Unimplemented(format!("can't handle function with {:?} ABI", abi))),
        }
    }

    fn read_discriminant_value(&self, adt_ptr: Pointer, adt_ty: Ty<'tcx>) -> EvalResult<'tcx, u128> {
        use rustc::ty::layout::Layout::*;
        let adt_layout = self.type_layout(adt_ty)?;
        trace!("read_discriminant_value {:#?}", adt_layout);

        let discr_val = match *adt_layout {
            General { discr, .. } | CEnum { discr, signed: false, .. } => {
                let discr_size = discr.size().bytes();
                self.memory.read_uint(adt_ptr, discr_size)?
            }

            CEnum { discr, signed: true, .. } => {
                let discr_size = discr.size().bytes();
                self.memory.read_int(adt_ptr, discr_size)? as u128
            }

            RawNullablePointer { nndiscr, value } => {
                let discr_size = value.size(&self.tcx.data_layout).bytes();
                trace!("rawnullablepointer with size {}", discr_size);
                self.read_nonnull_discriminant_value(adt_ptr, nndiscr as u128, discr_size)?
            }

            StructWrappedNullablePointer { nndiscr, ref discrfield, .. } => {
                let (offset, ty) = self.nonnull_offset_and_ty(adt_ty, nndiscr, discrfield)?;
                let nonnull = adt_ptr.offset(offset.bytes());
                trace!("struct wrapped nullable pointer type: {}", ty);
                // only the pointer part of a fat pointer is used for this space optimization
                let discr_size = self.type_size(ty)?.expect("bad StructWrappedNullablePointer discrfield");
                self.read_nonnull_discriminant_value(nonnull, nndiscr as u128, discr_size)?
            }

            // The discriminant_value intrinsic returns 0 for non-sum types.
            Array { .. } | FatPointer { .. } | Scalar { .. } | Univariant { .. } |
            Vector { .. } | UntaggedUnion { .. } => 0,
        };

        Ok(discr_val)
    }

    fn read_nonnull_discriminant_value(&self, ptr: Pointer, nndiscr: u128, discr_size: u64) -> EvalResult<'tcx, u128> {
        trace!("read_nonnull_discriminant_value: {:?}, {}, {}", ptr, nndiscr, discr_size);
        let not_null = match self.memory.read_uint(ptr, discr_size) {
            Ok(0) => false,
            Ok(_) | Err(EvalError::ReadPointerAsBytes) => true,
            Err(e) => return Err(e),
        };
        assert!(nndiscr == 0 || nndiscr == 1);
        Ok(if not_null { nndiscr } else { 1 - nndiscr })
    }

    fn call_c_abi(
        &mut self,
        def_id: DefId,
        args: &[mir::Operand<'tcx>],
        dest: Lvalue<'tcx>,
        dest_ty: Ty<'tcx>,
    ) -> EvalResult<'tcx> {
        let name = self.tcx.item_name(def_id);
        let attrs = self.tcx.get_attrs(def_id);
        let link_name = attr::first_attr_value_str_by_name(&attrs, "link_name")
            .unwrap_or(name)
            .as_str();

        let args_res: EvalResult<Vec<Value>> = args.iter()
            .map(|arg| self.eval_operand(arg))
            .collect();
        let args = args_res?;

        let usize = self.tcx.types.usize;

        match &link_name[..] {
            "__rust_allocate" => {
                let size = self.value_to_primval(args[0], usize)?.to_u64()?;
                let align = self.value_to_primval(args[1], usize)?.to_u64()?;
                let ptr = self.memory.allocate(size, align)?;
                self.write_primval(dest, PrimVal::Ptr(ptr), dest_ty)?;
            }

            "__rust_deallocate" => {
                let ptr = args[0].read_ptr(&self.memory)?;
                // FIXME: insert sanity check for size and align?
                let _old_size = self.value_to_primval(args[1], usize)?.to_u64()?;
                let _align = self.value_to_primval(args[2], usize)?.to_u64()?;
                self.memory.deallocate(ptr)?;
            },

            "__rust_reallocate" => {
                let ptr = args[0].read_ptr(&self.memory)?;
                let size = self.value_to_primval(args[2], usize)?.to_u64()?;
                let align = self.value_to_primval(args[3], usize)?.to_u64()?;
                let new_ptr = self.memory.reallocate(ptr, size, align)?;
                self.write_primval(dest, PrimVal::Ptr(new_ptr), dest_ty)?;
            }

            "memcmp" => {
                let left = args[0].read_ptr(&self.memory)?;
                let right = args[1].read_ptr(&self.memory)?;
                let n = self.value_to_primval(args[2], usize)?.to_u64()?;

                let result = {
                    let left_bytes = self.memory.read_bytes(left, n)?;
                    let right_bytes = self.memory.read_bytes(right, n)?;

                    use std::cmp::Ordering::*;
                    match left_bytes.cmp(right_bytes) {
                        Less => -1i8,
                        Equal => 0,
                        Greater => 1,
                    }
                };

                self.write_primval(dest, PrimVal::Bytes(result as u128), dest_ty)?;
            }

            "memrchr" => {
                let ptr = args[0].read_ptr(&self.memory)?;
                let val = self.value_to_primval(args[1], usize)?.to_u64()? as u8;
                let num = self.value_to_primval(args[2], usize)?.to_u64()?;
                if let Some(idx) = self.memory.read_bytes(ptr, num)?.iter().rev().position(|&c| c == val) {
                    let new_ptr = ptr.offset(num - idx as u64 - 1);
                    self.write_value(Value::ByVal(PrimVal::Ptr(new_ptr)), dest, dest_ty)?;
                } else {
                    self.write_value(Value::ByVal(PrimVal::Bytes(0)), dest, dest_ty)?;
                }
            }

            "memchr" => {
                let ptr = args[0].read_ptr(&self.memory)?;
                let val = self.value_to_primval(args[1], usize)?.to_u64()? as u8;
                let num = self.value_to_primval(args[2], usize)?.to_u64()?;
                if let Some(idx) = self.memory.read_bytes(ptr, num)?.iter().position(|&c| c == val) {
                    let new_ptr = ptr.offset(idx as u64);
                    self.write_value(Value::ByVal(PrimVal::Ptr(new_ptr)), dest, dest_ty)?;
                } else {
                    self.write_value(Value::ByVal(PrimVal::Bytes(0)), dest, dest_ty)?;
                }
            }

            "getenv" => {
                {
                    let name_ptr = args[0].read_ptr(&self.memory)?;
                    let name = self.memory.read_c_str(name_ptr)?;
                    info!("ignored env var request for `{:?}`", ::std::str::from_utf8(name));
                }
                self.write_value(Value::ByVal(PrimVal::Bytes(0)), dest, dest_ty)?;
            }

            // unix panic code inside libstd will read the return value of this function
            "pthread_rwlock_rdlock" => {
                self.write_primval(dest, PrimVal::Bytes(0), dest_ty)?;
            }

            link_name if link_name.starts_with("pthread_") => {
                warn!("ignoring C ABI call: {}", link_name);
                return Ok(());
            },

            _ => {
                return Err(EvalError::Unimplemented(format!("can't call C ABI function: {}", link_name)));
            }
        }

        // Since we pushed no stack frame, the main loop will act
        // as if the call just completed and it's returning to the
        // current frame.
        Ok(())
    }

    pub(super) fn fulfill_obligation(&self, trait_ref: ty::PolyTraitRef<'tcx>) -> traits::Vtable<'tcx, ()> {
        // Do the initial selection for the obligation. This yields the shallow result we are
        // looking for -- that is, what specific impl.
        self.tcx.infer_ctxt((), Reveal::All).enter(|infcx| {
            let mut selcx = traits::SelectionContext::new(&infcx);

            let obligation = traits::Obligation::new(
                traits::ObligationCause::misc(DUMMY_SP, ast::DUMMY_NODE_ID),
                trait_ref.to_poly_trait_predicate(),
            );
            let selection = selcx.select(&obligation).unwrap().unwrap();

            // Currently, we use a fulfillment context to completely resolve all nested obligations.
            // This is because they can inform the inference of the impl's type parameters.
            let mut fulfill_cx = traits::FulfillmentContext::new();
            let vtable = selection.map(|predicate| {
                fulfill_cx.register_predicate_obligation(&infcx, predicate);
            });
            infcx.drain_fulfillment_cx_or_panic(DUMMY_SP, &mut fulfill_cx, &vtable)
        })
    }

    fn unpack_fn_args(&self, args: &mut Vec<(Value, Ty<'tcx>)>) -> EvalResult<'tcx> {
        if let Some((last, last_ty)) = args.pop() {
            let last_layout = self.type_layout(last_ty)?;
            match (&last_ty.sty, last_layout) {
                (&ty::TyTuple(fields, _),
                 &Layout::Univariant { ref variant, .. }) => {
                    let offsets = variant.offsets.iter().map(|s| s.bytes());
                    let last_ptr = match last {
                        Value::ByRef(ptr) => ptr,
                        _ => bug!("rust-call ABI tuple argument wasn't Value::ByRef"),
                    };
                    for (offset, ty) in offsets.zip(fields) {
                        let arg = Value::ByRef(last_ptr.offset(offset));
                        args.push((arg, ty));
                    }
                }
                ty => bug!("expected tuple as last argument in function with 'rust-call' ABI, got {:?}", ty),
            }
        }
        Ok(())
    }

    /// Trait method, which has to be resolved to an impl method.
    fn trait_method(
        &mut self,
        trait_id: DefId,
        def_id: DefId,
        substs: &'tcx Substs<'tcx>,
        args: &mut Vec<(Value, Ty<'tcx>)>,
    ) -> EvalResult<'tcx, (DefId, &'tcx Substs<'tcx>, Vec<Pointer>)> {
        let trait_ref = ty::TraitRef::from_method(self.tcx, trait_id, substs);
        let trait_ref = self.tcx.normalize_associated_type(&ty::Binder(trait_ref));

        match self.fulfill_obligation(trait_ref) {
            traits::VtableImpl(vtable_impl) => {
                let impl_did = vtable_impl.impl_def_id;
                let mname = self.tcx.item_name(def_id);
                // Create a concatenated set of substitutions which includes those from the impl
                // and those from the method:
                let (did, substs) = find_method(self.tcx, substs, impl_did, vtable_impl.substs, mname);

                Ok((did, substs, Vec::new()))
            }

            traits::VtableClosure(vtable_closure) => {
                let trait_closure_kind = self.tcx
                    .lang_items
                    .fn_trait_kind(trait_id)
                    .expect("The substitutions should have no type parameters remaining after passing through fulfill_obligation");
                let closure_kind = self.tcx.closure_kind(vtable_closure.closure_def_id);
                trace!("closures {:?}, {:?}", closure_kind, trait_closure_kind);
                self.unpack_fn_args(args)?;
                let mut temporaries = Vec::new();
                match (closure_kind, trait_closure_kind) {
                    (ty::ClosureKind::Fn, ty::ClosureKind::Fn) |
                    (ty::ClosureKind::FnMut, ty::ClosureKind::FnMut) |
                    (ty::ClosureKind::FnOnce, ty::ClosureKind::FnOnce) |
                    (ty::ClosureKind::Fn, ty::ClosureKind::FnMut) => {} // No adapter needed.

                    (ty::ClosureKind::Fn, ty::ClosureKind::FnOnce) |
                    (ty::ClosureKind::FnMut, ty::ClosureKind::FnOnce) => {
                        // The closure fn is a `fn(&self, ...)` or `fn(&mut self, ...)`.
                        // We want a `fn(self, ...)`.
                        // We can produce this by doing something like:
                        //
                        //     fn call_once(self, ...) { call_mut(&self, ...) }
                        //     fn call_once(mut self, ...) { call_mut(&mut self, ...) }
                        //
                        // These are both the same at trans time.

                        // Interpreter magic: insert an intermediate pointer, so we can skip the
                        // intermediate function call.
                        let ptr = match args[0].0 {
                            Value::ByRef(ptr) => ptr,
                            Value::ByVal(primval) => {
                                let ptr = self.alloc_ptr(args[0].1)?;
                                let size = self.type_size(args[0].1)?.expect("closures are sized");
                                self.memory.write_primval(ptr, primval, size)?;
                                temporaries.push(ptr);
                                ptr
                            },
                            Value::ByValPair(a, b) => {
                                let ptr = self.alloc_ptr(args[0].1)?;
                                self.write_pair_to_ptr(a, b, ptr, args[0].1)?;
                                temporaries.push(ptr);
                                ptr
                            },
                        };
                        args[0].0 = Value::ByVal(PrimVal::Ptr(ptr));
                        args[0].1 = self.tcx.mk_mut_ptr(args[0].1);
                    }

                    _ => bug!("cannot convert {:?} to {:?}", closure_kind, trait_closure_kind),
                }
                Ok((vtable_closure.closure_def_id, vtable_closure.substs.substs, temporaries))
            }

            traits::VtableFnPointer(vtable_fn_ptr) => {
                if let ty::TyFnDef(did, substs, _) = vtable_fn_ptr.fn_ty.sty {
                    args.remove(0);
                    self.unpack_fn_args(args)?;
                    Ok((did, substs, Vec::new()))
                } else {
                    bug!("VtableFnPointer did not contain a concrete function: {:?}", vtable_fn_ptr)
                }
            }

            traits::VtableObject(ref data) => {
                let idx = self.tcx.get_vtable_index_of_object_method(data, def_id) as u64;
                if args.is_empty() {
                    return Err(EvalError::VtableForArgumentlessMethod);
                }
                let (self_ptr, vtable) = args[0].0.expect_ptr_vtable_pair(&self.memory)?;
                let idx = idx + 3;
                let offset = idx * self.memory.pointer_size();
                let fn_ptr = self.memory.read_ptr(vtable.offset(offset))?;
                trace!("args: {:#?}", args);
                match self.memory.get_fn(fn_ptr.alloc_id)? {
                    Function::FnDefAsTraitObject(fn_def) => {
                        trace!("sig: {:#?}", fn_def.sig);
                        assert!(fn_def.abi != abi::Abi::RustCall);
                        assert_eq!(args.len(), 2);
                        // a function item turned into a closure trait object
                        // the first arg is just there to give use the vtable
                        args.remove(0);
                        self.unpack_fn_args(args)?;
                        Ok((fn_def.def_id, fn_def.substs, Vec::new()))
                    },
                    Function::DropGlue(_) => Err(EvalError::ManuallyCalledDropGlue),
                    Function::Concrete(fn_def) => {
                        trace!("sig: {:#?}", fn_def.sig);
                        args[0] = (
                            Value::ByVal(PrimVal::Ptr(self_ptr)),
                            fn_def.sig.inputs()[0],
                        );
                        Ok((fn_def.def_id, fn_def.substs, Vec::new()))
                    },
                    Function::Closure(fn_def) => {
                        self.unpack_fn_args(args)?;
                        Ok((fn_def.def_id, fn_def.substs, Vec::new()))
                    }
                    Function::FnPtrAsTraitObject(sig) => {
                        trace!("sig: {:#?}", sig);
                        // the first argument was the fat ptr
                        args.remove(0);
                        self.unpack_fn_args(args)?;
                        let fn_ptr = self.memory.read_ptr(self_ptr)?;
                        let fn_def = self.memory.get_fn(fn_ptr.alloc_id)?.expect_concrete()?;
                        assert_eq!(sig, fn_def.sig);
                        Ok((fn_def.def_id, fn_def.substs, Vec::new()))
                    }
                }
            },
            vtable => bug!("resolved vtable bad vtable {:?} in trans", vtable),
        }
    }
}

#[derive(Debug)]
pub(super) struct ImplMethod<'tcx> {
    pub(super) method: ty::AssociatedItem,
    pub(super) substs: &'tcx Substs<'tcx>,
    pub(super) is_provided: bool,
}

/// Locates the applicable definition of a method, given its name.
pub(super) fn get_impl_method<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    substs: &'tcx Substs<'tcx>,
    impl_def_id: DefId,
    impl_substs: &'tcx Substs<'tcx>,
    name: ast::Name,
) -> ImplMethod<'tcx> {
    assert!(!substs.needs_infer());

    let trait_def_id = tcx.trait_id_of_impl(impl_def_id).unwrap();
    let trait_def = tcx.lookup_trait_def(trait_def_id);

    match trait_def.ancestors(impl_def_id).defs(tcx, name, ty::AssociatedKind::Method).next() {
        Some(node_item) => {
            let substs = tcx.infer_ctxt((), Reveal::All).enter(|infcx| {
                let substs = substs.rebase_onto(tcx, trait_def_id, impl_substs);
                let substs = traits::translate_substs(&infcx, impl_def_id,
                                                      substs, node_item.node);
                tcx.lift(&substs).unwrap_or_else(|| {
                    bug!("trans::meth::get_impl_method: translate_substs \
                          returned {:?} which contains inference types/regions",
                         substs);
                })
            });
            ImplMethod {
                method: node_item.item,
                substs,
                is_provided: node_item.node.is_from_trait(),
            }
        }
        None => {
            bug!("method {:?} not found in {:?}", name, impl_def_id)
        }
    }
}

/// Locates the applicable definition of a method, given its name.
pub fn find_method<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                             substs: &'tcx Substs<'tcx>,
                             impl_def_id: DefId,
                             impl_substs: &'tcx Substs<'tcx>,
                             name: ast::Name)
                             -> (DefId, &'tcx Substs<'tcx>)
{
    assert!(!substs.needs_infer());

    let trait_def_id = tcx.trait_id_of_impl(impl_def_id).unwrap();
    let trait_def = tcx.lookup_trait_def(trait_def_id);

    match trait_def.ancestors(impl_def_id).defs(tcx, name, ty::AssociatedKind::Method).next() {
        Some(node_item) => {
            let substs = tcx.infer_ctxt((), Reveal::All).enter(|infcx| {
                let substs = substs.rebase_onto(tcx, trait_def_id, impl_substs);
                let substs = traits::translate_substs(&infcx, impl_def_id, substs, node_item.node);
                tcx.lift(&substs).unwrap_or_else(|| {
                    bug!("find_method: translate_substs \
                          returned {:?} which contains inference types/regions",
                         substs);
                })
            });
            (node_item.item.def_id, substs)
        }
        None => {
            bug!("method {:?} not found in {:?}", name, impl_def_id)
        }
    }
}
