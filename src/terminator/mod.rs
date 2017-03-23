use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::ty::{self, Ty};
use rustc::ty::layout::Layout;
use syntax::codemap::Span;
use syntax::attr;
use syntax::abi::Abi;

use error::{EvalError, EvalResult};
use eval_context::{EvalContext, IntegerExt, StackPopCleanup, is_inhabited};
use lvalue::Lvalue;
use memory::Pointer;
use value::PrimVal;
use value::Value;

mod drop;
mod intrinsic;

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

            SwitchInt { ref discr, ref values, ref targets, .. } => {
                let discr_val = self.eval_operand(discr)?;
                let discr_ty = self.operand_ty(discr);
                let discr_prim = self.value_to_primval(discr_val, discr_ty)?;

                // Branch to the `otherwise` case by default, if no match is found.
                let mut target_block = targets[targets.len() - 1];

                for (index, const_int) in values.iter().enumerate() {
                    let prim = PrimVal::Bytes(const_int.to_u128_unchecked());
                    if discr_prim.to_bytes()? == prim.to_bytes()? {
                        target_block = targets[index];
                        break;
                    }
                }

                self.goto_block(target_block);
            }

            Call { ref func, ref args, ref destination, .. } => {
                let destination = match *destination {
                    Some((ref lv, target)) => Some((self.eval_lvalue(lv)?, target)),
                    None => None,
                };

                let func_ty = self.operand_ty(func);
                let (fn_def, sig) = match func_ty.sty {
                    ty::TyFnPtr(sig) => {
                        let fn_ptr = self.eval_operand_to_primval(func)?.to_ptr()?;
                        (self.memory.get_fn(fn_ptr.alloc_id)?, sig)
                    },
                    ty::TyFnDef(def_id, substs, sig) => (::eval_context::resolve(self.tcx, def_id, substs), sig),
                    _ => {
                        let msg = format!("can't handle callee of type {:?}", func_ty);
                        return Err(EvalError::Unimplemented(msg));
                    }
                };
                let sig = self.erase_lifetimes(&sig);
                self.eval_fn_call(fn_def, destination, args, terminator.source_info.span, sig)?;
            }

            Drop { ref location, target, .. } => {
                trace!("TerminatorKind::drop: {:?}, {:?}", location, self.substs());
                let lval = self.eval_lvalue(location)?;
                let ty = self.lvalue_ty(location);
                self.goto_block(target);
                let ty = ::eval_context::apply_param_substs(self.tcx, self.substs(), &ty);

                let instance = ::eval_context::resolve_drop_in_place(self.tcx, ty);
                self.drop_lvalue(lval, instance, ty, terminator.source_info.span)?;
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
        instance: ty::Instance<'tcx>,
        destination: Option<(Lvalue<'tcx>, mir::BasicBlock)>,
        arg_operands: &[mir::Operand<'tcx>],
        span: Span,
        sig: ty::FnSig<'tcx>,
    ) -> EvalResult<'tcx> {
        trace!("eval_fn_call: {:#?}", instance);
        match instance.def {
            ty::InstanceDef::Intrinsic(..) => {
                let (ret, target) = match destination {
                    Some(dest) => dest,
                    _ => return Err(EvalError::Unreachable),
                };
                let ty = sig.output();
                if !is_inhabited(self.tcx, ty) {
                    return Err(EvalError::Unreachable);
                }
                let layout = self.type_layout(ty)?;
                self.call_intrinsic(instance, arg_operands, ret, ty, layout, target)?;
                self.dump_local(ret);
                Ok(())
            },
            ty::InstanceDef::ClosureOnceShim{..} => {
                let mut args = Vec::new();
                for arg in arg_operands {
                    let arg_val = self.eval_operand(arg)?;
                    let arg_ty = self.operand_ty(arg);
                    args.push((arg_val, arg_ty));
                }
                assert_eq!(sig.abi, Abi::RustCall);
                self.eval_fn_call_inner(
                    instance,
                    destination,
                    args,
                    span,
                )
            }
            ty::InstanceDef::Item(_) => {
                let mut args = Vec::new();
                for arg in arg_operands {
                    let arg_val = self.eval_operand(arg)?;
                    let arg_ty = self.operand_ty(arg);
                    args.push((arg_val, arg_ty));
                }
                match sig.abi {
                    Abi::C => {
                        let ty = sig.output();
                        let (ret, target) = destination.unwrap();
                        self.call_c_abi(instance.def_id(), arg_operands, ret, ty)?;
                        self.dump_local(ret);
                        self.goto_block(target);
                        return Ok(());
                    },
                    Abi::Rust => {},
                    Abi::RustCall => self.unpack_fn_args(&mut args)?,
                    _ => unimplemented!(),
                }
                self.eval_fn_call_inner(
                    instance,
                    destination,
                    args,
                    span,
                )
            },
            ty::InstanceDef::DropGlue(..) => {
                assert_eq!(arg_operands.len(), 1);
                assert_eq!(sig.abi, Abi::Rust);
                let val = self.eval_operand(&arg_operands[0])?;
                let ty = self.operand_ty(&arg_operands[0]);
                let (_, target) = destination.expect("diverging drop glue");
                self.goto_block(target);
                // FIXME: deduplicate these matches
                let pointee_type = match ty.sty {
                    ty::TyRawPtr(ref tam) |
                    ty::TyRef(_, ref tam) => tam.ty,
                    ty::TyAdt(ref def, _) if def.is_box() => ty.boxed_ty(),
                    _ => bug!("can only deref pointer types"),
                };
                self.drop(val, instance, pointee_type, span)
            },
            ty::InstanceDef::FnPtrShim(..) => {
                let mut args = Vec::new();
                for arg in arg_operands {
                    let arg_val = self.eval_operand(arg)?;
                    let arg_ty = self.operand_ty(arg);
                    args.push((arg_val, arg_ty));
                }
                match sig.abi {
                    Abi::Rust => {
                        args.remove(0);
                    },
                    Abi::RustCall => {},
                    _ => unimplemented!(),
                }
                trace!("ABI: {}", sig.abi);
                self.eval_fn_call_inner(
                    instance,
                    destination,
                    args,
                    span,
                )
            }
            _ => Err(EvalError::Unimplemented(format!("can't handle function with {:?} ABI", sig.abi))),
        }
    }

    fn eval_fn_call_inner(
        &mut self,
        instance: ty::Instance<'tcx>,
        destination: Option<(Lvalue<'tcx>, mir::BasicBlock)>,
        args: Vec<(Value, Ty<'tcx>)>,
        span: Span,
    ) -> EvalResult<'tcx> {
        trace!("eval_fn_call_inner: {:#?}, {:#?}, {:#?}", instance, args, destination);

        // Only trait methods can have a Self parameter.

        let mir = match self.load_mir(instance.def) {
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
            instance,
            span,
            mir,
            return_lvalue,
            return_to_block,
        )?;

        let arg_locals = self.frame().mir.args_iter();
        assert_eq!(self.frame().mir.arg_count, args.len());
        for (arg_local, (arg_val, arg_ty)) in arg_locals.zip(args) {
            let dest = self.eval_lvalue(&mir::Lvalue::Local(arg_local))?;
            self.write_value(arg_val, dest, arg_ty)?;
        }

        Ok(())
    }

    pub fn read_discriminant_value(&self, adt_ptr: Pointer, adt_ty: Ty<'tcx>) -> EvalResult<'tcx, u128> {
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

    pub(crate) fn unpack_fn_args(&self, args: &mut Vec<(Value, Ty<'tcx>)>) -> EvalResult<'tcx> {
        if let Some((last, last_ty)) = args.pop() {
            let last_layout = self.type_layout(last_ty)?;
            match (&last_ty.sty, last_layout) {
                (&ty::TyTuple(fields, _),
                 &Layout::Univariant { ref variant, .. }) => {
                    let offsets = variant.offsets.iter().map(|s| s.bytes());
                    match last {
                        Value::ByRef(last_ptr) => {
                            for (offset, ty) in offsets.zip(fields) {
                                let arg = Value::ByRef(last_ptr.offset(offset));
                                args.push((arg, ty));
                            }
                        },
                        // propagate undefs
                        undef @ Value::ByVal(PrimVal::Undef) => {
                            for field_ty in fields {
                                args.push((undef, field_ty));
                            }
                        },
                        _ => bug!("rust-call ABI tuple argument was {:?}, but {:?} were expected", last, fields),
                    }
                }
                ty => bug!("expected tuple as last argument in function with 'rust-call' ABI, got {:?}", ty),
            }
        }
        Ok(())
    }
}
