//! Validates the MIR to ensure that invariants are upheld.

use super::{MirPass, MirSource};
use rustc_middle::mir::visit::Visitor;
use rustc_middle::{
    mir::{
        BasicBlock, Body, Location, Operand, Rvalue, Statement, StatementKind, Terminator,
        TerminatorKind,
    },
    ty::{self, ParamEnv, TyCtxt},
};
use rustc_span::{def_id::DefId, Span, DUMMY_SP};

pub struct Validator {
    /// Describes at which point in the pipeline this validation is happening.
    pub when: String,
}

impl<'tcx> MirPass<'tcx> for Validator {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, source: MirSource<'tcx>, body: &mut Body<'tcx>) {
        let def_id = source.def_id();
        let param_env = tcx.param_env(def_id);
        TypeChecker { when: &self.when, def_id, body, tcx, param_env }.visit_body(body);
    }
}

struct TypeChecker<'a, 'tcx> {
    when: &'a str,
    def_id: DefId,
    body: &'a Body<'tcx>,
    tcx: TyCtxt<'tcx>,
    param_env: ParamEnv<'tcx>,
}

impl<'a, 'tcx> TypeChecker<'a, 'tcx> {
    fn fail(&self, span: Span, msg: impl AsRef<str>) {
        // We use `delay_span_bug` as we might see broken MIR when other errors have already
        // occurred.
        self.tcx.sess.diagnostic().delay_span_bug(
            span,
            &format!("broken MIR in {:?} ({}): {}", self.def_id, self.when, msg.as_ref()),
        );
    }

    fn check_bb(&self, span: Span, bb: BasicBlock) {
        if self.body.basic_blocks().get(bb).is_none() {
            self.fail(span, format!("encountered jump to invalid basic block {:?}", bb))
        }
    }
}

impl<'a, 'tcx> Visitor<'tcx> for TypeChecker<'a, 'tcx> {
    fn visit_operand(&mut self, operand: &Operand<'tcx>, location: Location) {
        // `Operand::Copy` is only supposed to be used with `Copy` types.
        if let Operand::Copy(place) = operand {
            let ty = place.ty(&self.body.local_decls, self.tcx).ty;

            if !ty.is_copy_modulo_regions(self.tcx, self.param_env, DUMMY_SP) {
                self.fail(
                    DUMMY_SP,
                    format!("`Operand::Copy` with non-`Copy` type {} at {:?}", ty, location),
                );
            }
        }

        self.super_operand(operand, location);
    }

    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        // The sides of an assignment must not alias. Currently this just checks whether the places
        // are identical.
        if let StatementKind::Assign(box (dest, rvalue)) = &statement.kind {
            match rvalue {
                Rvalue::Use(Operand::Copy(src) | Operand::Move(src)) => {
                    if dest == src {
                        self.fail(
                            DUMMY_SP,
                            format!(
                                "encountered `Assign` statement with overlapping memory at {:?}",
                                location
                            ),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                self.check_bb(terminator.source_info.span, *target);
            }
            TerminatorKind::SwitchInt { targets, .. } => {
                if targets.is_empty() {
                    self.fail(
                        terminator.source_info.span,
                        "encountered `SwitchInt` terminator with no target to jump to",
                    );
                }
                for target in targets {
                    self.check_bb(terminator.source_info.span, *target);
                }
            }
            TerminatorKind::Drop { target, unwind, .. } => {
                self.check_bb(terminator.source_info.span, *target);
                if let Some(unwind) = unwind {
                    self.check_bb(terminator.source_info.span, *unwind);
                }
            }
            TerminatorKind::DropAndReplace { target, unwind, .. } => {
                self.check_bb(terminator.source_info.span, *target);
                if let Some(unwind) = unwind {
                    self.check_bb(terminator.source_info.span, *unwind);
                }
            }
            TerminatorKind::Call { func, destination, cleanup, .. } => {
                let func_ty = func.ty(&self.body.local_decls, self.tcx);
                match func_ty.kind {
                    ty::FnPtr(..) | ty::FnDef(..) => {}
                    _ => self.fail(
                        terminator.source_info.span,
                        format!("encountered non-callable type {} in `Call` terminator", func_ty),
                    ),
                }
                if let Some((_, target)) = destination {
                    self.check_bb(terminator.source_info.span, *target);
                }
                if let Some(cleanup) = cleanup {
                    self.check_bb(terminator.source_info.span, *cleanup);
                }
            }
            TerminatorKind::Assert { cond, target, cleanup, .. } => {
                let cond_ty = cond.ty(&self.body.local_decls, self.tcx);
                if cond_ty != self.tcx.types.bool {
                    self.fail(
                        terminator.source_info.span,
                        format!(
                            "encountered non-boolean condition of type {} in `Assert` terminator",
                            cond_ty
                        ),
                    );
                }
                self.check_bb(terminator.source_info.span, *target);
                if let Some(cleanup) = cleanup {
                    self.check_bb(terminator.source_info.span, *cleanup);
                }
            }
            TerminatorKind::Yield { resume, drop, .. } => {
                self.check_bb(terminator.source_info.span, *resume);
                if let Some(drop) = drop {
                    self.check_bb(terminator.source_info.span, *drop);
                }
            }
            TerminatorKind::FalseEdges { real_target, imaginary_target } => {
                self.check_bb(terminator.source_info.span, *real_target);
                self.check_bb(terminator.source_info.span, *imaginary_target);
            }
            TerminatorKind::FalseUnwind { real_target, unwind } => {
                self.check_bb(terminator.source_info.span, *real_target);
                if let Some(unwind) = unwind {
                    self.check_bb(terminator.source_info.span, *unwind);
                }
            }
            TerminatorKind::InlineAsm { destination, .. } => {
                if let Some(destination) = destination {
                    self.check_bb(terminator.source_info.span, *destination);
                }
            }
            // Nothing to validate for these.
            TerminatorKind::Resume
            | TerminatorKind::Abort
            | TerminatorKind::Return
            | TerminatorKind::Unreachable
            | TerminatorKind::GeneratorDrop => {}
        }
    }
}
