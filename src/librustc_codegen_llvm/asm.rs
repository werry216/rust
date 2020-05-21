use crate::builder::Builder;
use crate::context::CodegenCx;
use crate::llvm;
use crate::type_::Type;
use crate::type_of::LayoutLlvmExt;
use crate::value::Value;

use rustc_ast::ast::LlvmAsmDialect;
use rustc_ast::ast::{InlineAsmOptions, InlineAsmTemplatePiece};
use rustc_codegen_ssa::mir::operand::OperandValue;
use rustc_codegen_ssa::mir::place::PlaceRef;
use rustc_codegen_ssa::traits::*;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir as hir;
use rustc_middle::span_bug;
use rustc_middle::ty::layout::TyAndLayout;
use rustc_span::Span;
use rustc_target::abi::*;
use rustc_target::asm::*;

use libc::{c_char, c_uint};
use log::debug;

impl AsmBuilderMethods<'tcx> for Builder<'a, 'll, 'tcx> {
    fn codegen_llvm_inline_asm(
        &mut self,
        ia: &hir::LlvmInlineAsmInner,
        outputs: Vec<PlaceRef<'tcx, &'ll Value>>,
        mut inputs: Vec<&'ll Value>,
        span: Span,
    ) -> bool {
        let mut ext_constraints = vec![];
        let mut output_types = vec![];

        // Prepare the output operands
        let mut indirect_outputs = vec![];
        for (i, (out, &place)) in ia.outputs.iter().zip(&outputs).enumerate() {
            if out.is_rw {
                let operand = self.load_operand(place);
                if let OperandValue::Immediate(_) = operand.val {
                    inputs.push(operand.immediate());
                }
                ext_constraints.push(i.to_string());
            }
            if out.is_indirect {
                let operand = self.load_operand(place);
                if let OperandValue::Immediate(_) = operand.val {
                    indirect_outputs.push(operand.immediate());
                }
            } else {
                output_types.push(place.layout.llvm_type(self.cx));
            }
        }
        if !indirect_outputs.is_empty() {
            indirect_outputs.extend_from_slice(&inputs);
            inputs = indirect_outputs;
        }

        let clobbers = ia.clobbers.iter().map(|s| format!("~{{{}}}", &s));

        // Default per-arch clobbers
        // Basically what clang does
        let arch_clobbers = match &self.sess().target.target.arch[..] {
            "x86" | "x86_64" => vec!["~{dirflag}", "~{fpsr}", "~{flags}"],
            "mips" | "mips64" => vec!["~{$1}"],
            _ => Vec::new(),
        };

        let all_constraints = ia
            .outputs
            .iter()
            .map(|out| out.constraint.to_string())
            .chain(ia.inputs.iter().map(|s| s.to_string()))
            .chain(ext_constraints)
            .chain(clobbers)
            .chain(arch_clobbers.iter().map(|s| (*s).to_string()))
            .collect::<Vec<String>>()
            .join(",");

        debug!("Asm Constraints: {}", &all_constraints);

        // Depending on how many outputs we have, the return type is different
        let num_outputs = output_types.len();
        let output_type = match num_outputs {
            0 => self.type_void(),
            1 => output_types[0],
            _ => self.type_struct(&output_types, false),
        };

        let asm = ia.asm.as_str();
        let r = inline_asm_call(
            self,
            &asm,
            &all_constraints,
            &inputs,
            output_type,
            ia.volatile,
            ia.alignstack,
            ia.dialect,
            span,
        );
        if r.is_none() {
            return false;
        }
        let r = r.unwrap();

        // Again, based on how many outputs we have
        let outputs = ia.outputs.iter().zip(&outputs).filter(|&(ref o, _)| !o.is_indirect);
        for (i, (_, &place)) in outputs.enumerate() {
            let v = if num_outputs == 1 { r } else { self.extract_value(r, i as u64) };
            OperandValue::Immediate(v).store(self, place);
        }

        true
    }

    fn codegen_inline_asm(
        &mut self,
        template: &[InlineAsmTemplatePiece],
        operands: &[InlineAsmOperandRef<'tcx, Self>],
        options: InlineAsmOptions,
        span: Span,
    ) {
        let asm_arch = self.tcx.sess.asm_arch.unwrap();

        // Collect the types of output operands
        let mut constraints = vec![];
        let mut output_types = vec![];
        let mut op_idx = FxHashMap::default();
        for (idx, op) in operands.iter().enumerate() {
            match *op {
                InlineAsmOperandRef::Out { reg, late, place } => {
                    let ty = if let Some(place) = place {
                        llvm_fixup_output_type(self.cx, reg.reg_class(), &place.layout)
                    } else {
                        // If the output is discarded, we don't really care what
                        // type is used. We're just using this to tell LLVM to
                        // reserve the register.
                        dummy_output_type(self.cx, reg.reg_class())
                    };
                    output_types.push(ty);
                    op_idx.insert(idx, constraints.len());
                    let prefix = if late { "=" } else { "=&" };
                    constraints.push(format!("{}{}", prefix, reg_to_llvm(reg)));
                }
                InlineAsmOperandRef::InOut { reg, late, in_value, out_place } => {
                    let ty = if let Some(ref out_place) = out_place {
                        llvm_fixup_output_type(self.cx, reg.reg_class(), &out_place.layout)
                    } else {
                        // LLVM required tied operands to have the same type,
                        // so we just use the type of the input.
                        llvm_fixup_output_type(self.cx, reg.reg_class(), &in_value.layout)
                    };
                    output_types.push(ty);
                    op_idx.insert(idx, constraints.len());
                    let prefix = if late { "=" } else { "=&" };
                    constraints.push(format!("{}{}", prefix, reg_to_llvm(reg)));
                }
                _ => {}
            }
        }

        // Collect input operands
        let mut inputs = vec![];
        for (idx, op) in operands.iter().enumerate() {
            match *op {
                InlineAsmOperandRef::In { reg, value } => {
                    let value =
                        llvm_fixup_input(self, value.immediate(), reg.reg_class(), &value.layout);
                    inputs.push(value);
                    op_idx.insert(idx, constraints.len());
                    constraints.push(reg_to_llvm(reg));
                }
                InlineAsmOperandRef::InOut { reg, late: _, in_value, out_place: _ } => {
                    let value = llvm_fixup_input(
                        self,
                        in_value.immediate(),
                        reg.reg_class(),
                        &in_value.layout,
                    );
                    inputs.push(value);
                    constraints.push(format!("{}", op_idx[&idx]));
                }
                InlineAsmOperandRef::SymFn { instance } => {
                    inputs.push(self.cx.get_fn(instance));
                    op_idx.insert(idx, constraints.len());
                    constraints.push("s".to_string());
                }
                InlineAsmOperandRef::SymStatic { def_id } => {
                    inputs.push(self.cx.get_static(def_id));
                    op_idx.insert(idx, constraints.len());
                    constraints.push("s".to_string());
                }
                _ => {}
            }
        }

        // Build the template string
        let mut template_str = String::new();
        for piece in template {
            match *piece {
                InlineAsmTemplatePiece::String(ref s) => {
                    if s.contains('$') {
                        for c in s.chars() {
                            if c == '$' {
                                template_str.push_str("$$");
                            } else {
                                template_str.push(c);
                            }
                        }
                    } else {
                        template_str.push_str(s)
                    }
                }
                InlineAsmTemplatePiece::Placeholder { operand_idx, modifier, span: _ } => {
                    match operands[operand_idx] {
                        InlineAsmOperandRef::In { reg, .. }
                        | InlineAsmOperandRef::Out { reg, .. }
                        | InlineAsmOperandRef::InOut { reg, .. } => {
                            let modifier = modifier_to_llvm(asm_arch, reg.reg_class(), modifier);
                            if let Some(modifier) = modifier {
                                template_str.push_str(&format!(
                                    "${{{}:{}}}",
                                    op_idx[&operand_idx], modifier
                                ));
                            } else {
                                template_str.push_str(&format!("${{{}}}", op_idx[&operand_idx]));
                            }
                        }
                        InlineAsmOperandRef::Const { ref string } => {
                            // Const operands get injected directly into the template
                            template_str.push_str(string);
                        }
                        InlineAsmOperandRef::SymFn { .. }
                        | InlineAsmOperandRef::SymStatic { .. } => {
                            // Only emit the raw symbol name
                            template_str.push_str(&format!("${{{}:c}}", op_idx[&operand_idx]));
                        }
                    }
                }
            }
        }

        if !options.contains(InlineAsmOptions::PRESERVES_FLAGS) {
            match asm_arch {
                InlineAsmArch::AArch64 | InlineAsmArch::Arm => {
                    constraints.push("~{cc}".to_string());
                }
                InlineAsmArch::X86 | InlineAsmArch::X86_64 => {
                    constraints.extend_from_slice(&[
                        "~{dirflag}".to_string(),
                        "~{fpsr}".to_string(),
                        "~{flags}".to_string(),
                    ]);
                }
                InlineAsmArch::RiscV32 | InlineAsmArch::RiscV64 => {}
                InlineAsmArch::Nvptx64 => {}
            }
        }
        if !options.contains(InlineAsmOptions::NOMEM) {
            // This is actually ignored by LLVM, but it's probably best to keep
            // it just in case. LLVM instead uses the ReadOnly/ReadNone
            // attributes on the call instruction to optimize.
            constraints.push("~{memory}".to_string());
        }
        let volatile = !options.contains(InlineAsmOptions::PURE);
        let alignstack = !options.contains(InlineAsmOptions::NOSTACK);
        let output_type = match &output_types[..] {
            [] => self.type_void(),
            [ty] => ty,
            tys => self.type_struct(&tys, false),
        };
        let dialect = match asm_arch {
            InlineAsmArch::X86 | InlineAsmArch::X86_64
                if !options.contains(InlineAsmOptions::ATT_SYNTAX) =>
            {
                LlvmAsmDialect::Intel
            }
            _ => LlvmAsmDialect::Att,
        };
        let result = inline_asm_call(
            self,
            &template_str,
            &constraints.join(","),
            &inputs,
            output_type,
            volatile,
            alignstack,
            dialect,
            span,
        )
        .unwrap_or_else(|| span_bug!(span, "LLVM asm constraint validation failed"));

        if options.contains(InlineAsmOptions::PURE) {
            if options.contains(InlineAsmOptions::NOMEM) {
                llvm::Attribute::ReadNone.apply_callsite(llvm::AttributePlace::Function, result);
            } else if options.contains(InlineAsmOptions::READONLY) {
                llvm::Attribute::ReadOnly.apply_callsite(llvm::AttributePlace::Function, result);
            }
        } else {
            if options.contains(InlineAsmOptions::NOMEM) {
                llvm::Attribute::InaccessibleMemOnly
                    .apply_callsite(llvm::AttributePlace::Function, result);
            } else {
                // LLVM doesn't have an attribute to represent ReadOnly + SideEffect
            }
        }

        // Write results to outputs
        for (idx, op) in operands.iter().enumerate() {
            if let InlineAsmOperandRef::Out { reg, place: Some(place), .. }
            | InlineAsmOperandRef::InOut { reg, out_place: Some(place), .. } = *op
            {
                let value = if output_types.len() == 1 {
                    result
                } else {
                    self.extract_value(result, op_idx[&idx] as u64)
                };
                let value = llvm_fixup_output(self, value, reg.reg_class(), &place.layout);
                OperandValue::Immediate(value).store(self, place);
            }
        }
    }
}

impl AsmMethods for CodegenCx<'ll, 'tcx> {
    fn codegen_global_asm(&self, ga: &hir::GlobalAsm) {
        let asm = ga.asm.as_str();
        unsafe {
            llvm::LLVMRustAppendModuleInlineAsm(self.llmod, asm.as_ptr().cast(), asm.len());
        }
    }
}

fn inline_asm_call(
    bx: &mut Builder<'a, 'll, 'tcx>,
    asm: &str,
    cons: &str,
    inputs: &[&'ll Value],
    output: &'ll llvm::Type,
    volatile: bool,
    alignstack: bool,
    dia: LlvmAsmDialect,
    span: Span,
) -> Option<&'ll Value> {
    let volatile = if volatile { llvm::True } else { llvm::False };
    let alignstack = if alignstack { llvm::True } else { llvm::False };

    let argtys = inputs
        .iter()
        .map(|v| {
            debug!("Asm Input Type: {:?}", *v);
            bx.cx.val_ty(*v)
        })
        .collect::<Vec<_>>();

    debug!("Asm Output Type: {:?}", output);
    let fty = bx.cx.type_func(&argtys[..], output);
    unsafe {
        // Ask LLVM to verify that the constraints are well-formed.
        let constraints_ok = llvm::LLVMRustInlineAsmVerify(fty, cons.as_ptr().cast(), cons.len());
        debug!("constraint verification result: {:?}", constraints_ok);
        if constraints_ok {
            let v = llvm::LLVMRustInlineAsm(
                fty,
                asm.as_ptr().cast(),
                asm.len(),
                cons.as_ptr().cast(),
                cons.len(),
                volatile,
                alignstack,
                llvm::AsmDialect::from_generic(dia),
            );
            let call = bx.call(v, inputs, None);

            // Store mark in a metadata node so we can map LLVM errors
            // back to source locations.  See #17552.
            let key = "srcloc";
            let kind = llvm::LLVMGetMDKindIDInContext(
                bx.llcx,
                key.as_ptr() as *const c_char,
                key.len() as c_uint,
            );

            let val: &'ll Value = bx.const_i32(span.ctxt().outer_expn().as_u32() as i32);
            llvm::LLVMSetMetadata(call, kind, llvm::LLVMMDNodeInContext(bx.llcx, &val, 1));

            Some(call)
        } else {
            // LLVM has detected an issue with our constraints, bail out
            None
        }
    }
}

/// Converts a register class to an LLVM constraint code.
fn reg_to_llvm(reg: InlineAsmRegOrRegClass) -> String {
    match reg {
        InlineAsmRegOrRegClass::Reg(reg) => format!("{{{}}}", reg.name()),
        InlineAsmRegOrRegClass::RegClass(reg) => match reg {
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::reg) => "r",
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg) => "w",
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16) => "x",
            InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg) => "r",
            InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg_thumb) => "l",
            InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg)
            | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low16)
            | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low8) => "t",
            InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg_low16)
            | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low8)
            | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low4) => "x",
            InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg)
            | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg) => "w",
            InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg16) => "h",
            InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg32) => "r",
            InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg64) => "l",
            InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::freg32) => "f",
            InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::freg64) => "d",
            InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::reg) => "r",
            InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::freg) => "f",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::reg) => "r",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd) => "Q",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_byte) => "q",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::xmm_reg)
            | InlineAsmRegClass::X86(X86InlineAsmRegClass::ymm_reg) => "x",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::zmm_reg) => "v",
            InlineAsmRegClass::X86(X86InlineAsmRegClass::kreg) => "^Yk",
        }
        .to_string(),
    }
}

/// Converts a modifier into LLVM's equivalent modifier.
fn modifier_to_llvm(
    arch: InlineAsmArch,
    reg: InlineAsmRegClass,
    modifier: Option<char>,
) -> Option<char> {
    match reg {
        InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::reg) => modifier,
        InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg)
        | InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16) => {
            if modifier == Some('v') { None } else { modifier }
        }
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg_thumb) => None,
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg_low16) => None,
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low16)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low8) => Some('P'),
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low8)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low4) => {
            if modifier.is_none() {
                Some('q')
            } else {
                modifier
            }
        }
        InlineAsmRegClass::Nvptx(_) => None,
        InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::reg)
        | InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::freg) => None,
        InlineAsmRegClass::X86(X86InlineAsmRegClass::reg)
        | InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd) => match modifier {
            None if arch == InlineAsmArch::X86_64 => Some('q'),
            None => Some('k'),
            Some('l') => Some('b'),
            Some('h') => Some('h'),
            Some('x') => Some('w'),
            Some('e') => Some('k'),
            Some('r') => Some('q'),
            _ => unreachable!(),
        },
        InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_byte) => None,
        InlineAsmRegClass::X86(reg @ X86InlineAsmRegClass::xmm_reg)
        | InlineAsmRegClass::X86(reg @ X86InlineAsmRegClass::ymm_reg)
        | InlineAsmRegClass::X86(reg @ X86InlineAsmRegClass::zmm_reg) => match (reg, modifier) {
            (X86InlineAsmRegClass::xmm_reg, None) => Some('x'),
            (X86InlineAsmRegClass::ymm_reg, None) => Some('t'),
            (X86InlineAsmRegClass::zmm_reg, None) => Some('g'),
            (_, Some('x')) => Some('x'),
            (_, Some('y')) => Some('t'),
            (_, Some('z')) => Some('g'),
            _ => unreachable!(),
        },
        InlineAsmRegClass::X86(X86InlineAsmRegClass::kreg) => None,
    }
}

/// Type to use for outputs that are discarded. It doesn't really matter what
/// the type is, as long as it is valid for the constraint code.
fn dummy_output_type(cx: &CodegenCx<'ll, 'tcx>, reg: InlineAsmRegClass) -> &'ll Type {
    match reg {
        InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::reg) => cx.type_i32(),
        InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg)
        | InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16) => {
            cx.type_vector(cx.type_i64(), 2)
        }
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::reg_thumb) => cx.type_i32(),
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::sreg_low16) => cx.type_f32(),
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low16)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::dreg_low8) => cx.type_f64(),
        InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low8)
        | InlineAsmRegClass::Arm(ArmInlineAsmRegClass::qreg_low4) => {
            cx.type_vector(cx.type_i64(), 2)
        }
        InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg16) => cx.type_i16(),
        InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg32) => cx.type_i32(),
        InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::reg64) => cx.type_i64(),
        InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::freg32) => cx.type_f32(),
        InlineAsmRegClass::Nvptx(NvptxInlineAsmRegClass::freg64) => cx.type_f64(),
        InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::reg) => cx.type_i32(),
        InlineAsmRegClass::RiscV(RiscVInlineAsmRegClass::freg) => cx.type_f32(),
        InlineAsmRegClass::X86(X86InlineAsmRegClass::reg)
        | InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd) => cx.type_i32(),
        InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_byte) => cx.type_i8(),
        InlineAsmRegClass::X86(X86InlineAsmRegClass::xmm_reg)
        | InlineAsmRegClass::X86(X86InlineAsmRegClass::ymm_reg)
        | InlineAsmRegClass::X86(X86InlineAsmRegClass::zmm_reg) => cx.type_f32(),
        InlineAsmRegClass::X86(X86InlineAsmRegClass::kreg) => cx.type_i16(),
    }
}

/// Helper function to get the LLVM type for a Scalar. Pointers are returned as
/// the equivalent integer type.
fn llvm_asm_scalar_type(cx: &CodegenCx<'ll, 'tcx>, scalar: &Scalar) -> &'ll Type {
    match scalar.value {
        Primitive::Int(Integer::I8, _) => cx.type_i8(),
        Primitive::Int(Integer::I16, _) => cx.type_i16(),
        Primitive::Int(Integer::I32, _) => cx.type_i32(),
        Primitive::Int(Integer::I64, _) => cx.type_i64(),
        Primitive::F32 => cx.type_f32(),
        Primitive::F64 => cx.type_f64(),
        Primitive::Pointer => cx.type_isize(),
        _ => unreachable!(),
    }
}

/// Fix up an input value to work around LLVM bugs.
fn llvm_fixup_input(
    bx: &mut Builder<'a, 'll, 'tcx>,
    mut value: &'ll Value,
    reg: InlineAsmRegClass,
    layout: &TyAndLayout<'tcx>,
) -> &'ll Value {
    match (reg, &layout.abi) {
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg), Abi::Scalar(s)) => {
            if let Primitive::Int(Integer::I8, _) = s.value {
                let vec_ty = bx.cx.type_vector(bx.cx.type_i8(), 8);
                bx.insert_element(bx.const_undef(vec_ty), value, bx.const_i32(0))
            } else {
                value
            }
        }
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16), Abi::Scalar(s)) => {
            let elem_ty = llvm_asm_scalar_type(bx.cx, s);
            let count = 16 / layout.size.bytes();
            let vec_ty = bx.cx.type_vector(elem_ty, count);
            if let Primitive::Pointer = s.value {
                value = bx.ptrtoint(value, bx.cx.type_isize());
            }
            bx.insert_element(bx.const_undef(vec_ty), value, bx.const_i32(0))
        }
        (
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16),
            Abi::Vector { element, count },
        ) if layout.size.bytes() == 8 => {
            let elem_ty = llvm_asm_scalar_type(bx.cx, element);
            let vec_ty = bx.cx.type_vector(elem_ty, *count);
            let indices: Vec<_> = (0..count * 2).map(|x| bx.const_i32(x as i32)).collect();
            bx.shuffle_vector(value, bx.const_undef(vec_ty), bx.const_vector(&indices))
        }
        (InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd), Abi::Scalar(s))
            if s.value == Primitive::F64 =>
        {
            bx.bitcast(value, bx.cx.type_i64())
        }
        (
            InlineAsmRegClass::X86(X86InlineAsmRegClass::xmm_reg | X86InlineAsmRegClass::zmm_reg),
            Abi::Vector { .. },
        ) if layout.size.bytes() == 64 => bx.bitcast(value, bx.cx.type_vector(bx.cx.type_f64(), 8)),
        (
            InlineAsmRegClass::Arm(
                ArmInlineAsmRegClass::sreg_low16
                | ArmInlineAsmRegClass::dreg_low8
                | ArmInlineAsmRegClass::qreg_low4
                | ArmInlineAsmRegClass::dreg
                | ArmInlineAsmRegClass::qreg,
            ),
            Abi::Scalar(s),
        ) => {
            if let Primitive::Int(Integer::I32, _) = s.value {
                bx.bitcast(value, bx.cx.type_f32())
            } else {
                value
            }
        }
        _ => value,
    }
}

/// Fix up an output value to work around LLVM bugs.
fn llvm_fixup_output(
    bx: &mut Builder<'a, 'll, 'tcx>,
    mut value: &'ll Value,
    reg: InlineAsmRegClass,
    layout: &TyAndLayout<'tcx>,
) -> &'ll Value {
    match (reg, &layout.abi) {
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg), Abi::Scalar(s)) => {
            if let Primitive::Int(Integer::I8, _) = s.value {
                bx.extract_element(value, bx.const_i32(0))
            } else {
                value
            }
        }
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16), Abi::Scalar(s)) => {
            value = bx.extract_element(value, bx.const_i32(0));
            if let Primitive::Pointer = s.value {
                value = bx.inttoptr(value, layout.llvm_type(bx.cx));
            }
            value
        }
        (
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16),
            Abi::Vector { element, count },
        ) if layout.size.bytes() == 8 => {
            let elem_ty = llvm_asm_scalar_type(bx.cx, element);
            let vec_ty = bx.cx.type_vector(elem_ty, *count * 2);
            let indices: Vec<_> = (0..*count).map(|x| bx.const_i32(x as i32)).collect();
            bx.shuffle_vector(value, bx.const_undef(vec_ty), bx.const_vector(&indices))
        }
        (InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd), Abi::Scalar(s))
            if s.value == Primitive::F64 =>
        {
            bx.bitcast(value, bx.cx.type_f64())
        }
        (
            InlineAsmRegClass::X86(X86InlineAsmRegClass::xmm_reg | X86InlineAsmRegClass::zmm_reg),
            Abi::Vector { .. },
        ) if layout.size.bytes() == 64 => bx.bitcast(value, layout.llvm_type(bx.cx)),
        (
            InlineAsmRegClass::Arm(
                ArmInlineAsmRegClass::sreg_low16
                | ArmInlineAsmRegClass::dreg_low8
                | ArmInlineAsmRegClass::qreg_low4
                | ArmInlineAsmRegClass::dreg
                | ArmInlineAsmRegClass::qreg,
            ),
            Abi::Scalar(s),
        ) => {
            if let Primitive::Int(Integer::I32, _) = s.value {
                bx.bitcast(value, bx.cx.type_i32())
            } else {
                value
            }
        }
        _ => value,
    }
}

/// Output type to use for llvm_fixup_output.
fn llvm_fixup_output_type(
    cx: &CodegenCx<'ll, 'tcx>,
    reg: InlineAsmRegClass,
    layout: &TyAndLayout<'tcx>,
) -> &'ll Type {
    match (reg, &layout.abi) {
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg), Abi::Scalar(s)) => {
            if let Primitive::Int(Integer::I8, _) = s.value {
                cx.type_vector(cx.type_i8(), 8)
            } else {
                layout.llvm_type(cx)
            }
        }
        (InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16), Abi::Scalar(s)) => {
            let elem_ty = llvm_asm_scalar_type(cx, s);
            let count = 16 / layout.size.bytes();
            cx.type_vector(elem_ty, count)
        }
        (
            InlineAsmRegClass::AArch64(AArch64InlineAsmRegClass::vreg_low16),
            Abi::Vector { element, count },
        ) if layout.size.bytes() == 8 => {
            let elem_ty = llvm_asm_scalar_type(cx, element);
            cx.type_vector(elem_ty, count * 2)
        }
        (InlineAsmRegClass::X86(X86InlineAsmRegClass::reg_abcd), Abi::Scalar(s))
            if s.value == Primitive::F64 =>
        {
            cx.type_i64()
        }
        (
            InlineAsmRegClass::X86(X86InlineAsmRegClass::xmm_reg | X86InlineAsmRegClass::zmm_reg),
            Abi::Vector { .. },
        ) if layout.size.bytes() == 64 => cx.type_vector(cx.type_f64(), 8),
        (
            InlineAsmRegClass::Arm(
                ArmInlineAsmRegClass::sreg_low16
                | ArmInlineAsmRegClass::dreg_low8
                | ArmInlineAsmRegClass::qreg_low4
                | ArmInlineAsmRegClass::dreg
                | ArmInlineAsmRegClass::qreg,
            ),
            Abi::Scalar(s),
        ) => {
            if let Primitive::Int(Integer::I32, _) = s.value {
                cx.type_f32()
            } else {
                layout.llvm_type(cx)
            }
        }
        _ => layout.llvm_type(cx),
    }
}
