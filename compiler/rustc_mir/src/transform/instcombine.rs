//! Performs various peephole optimizations.

use crate::transform::MirPass;
use rustc_hir::Mutability;
use rustc_middle::mir::{
    BinOp, Body, Constant, LocalDecls, Operand, Place, ProjectionElem, Rvalue, SourceInfo,
    StatementKind,
};
use rustc_middle::ty::{self, TyCtxt};

pub struct InstCombine;

impl<'tcx> MirPass<'tcx> for InstCombine {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let param_env = tcx.param_env(body.source.def_id());
        let (basic_blocks, local_decls) = body.basic_blocks_and_local_decls_mut();
        let ctx = InstCombineContext { tcx, local_decls, param_env };
        for block in basic_blocks.iter_mut() {
            for statement in block.statements.iter_mut() {
                ctx.combine_zst(&statement.source_info, &mut statement.kind);
                match statement.kind {
                    StatementKind::Assign(box (_place, ref mut rvalue)) => {
                        ctx.combine_bool_cmp(&statement.source_info, rvalue);
                        ctx.combine_ref_deref(&statement.source_info, rvalue);
                        ctx.combine_len(&statement.source_info, rvalue);
                    }
                    _ => {}
                }
            }
        }
    }
}

struct InstCombineContext<'tcx, 'a> {
    tcx: TyCtxt<'tcx>,
    local_decls: &'a LocalDecls<'tcx>,
    param_env: ty::ParamEnv<'tcx>,
}

impl<'tcx, 'a> InstCombineContext<'tcx, 'a> {
    fn should_combine(&self, source_info: &SourceInfo, rvalue: &Rvalue<'tcx>) -> bool {
        self.tcx.consider_optimizing(|| {
            format!("InstCombine - Rvalue: {:?} SourceInfo: {:?}", rvalue, source_info)
        })
    }

    /// Remove assignments to inhabited ZST places.
    fn combine_zst(&self, source_info: &SourceInfo, kind: &mut StatementKind<'tcx>) {
        match kind {
            StatementKind::Assign(box (place, _)) => {
                let place_ty = place.ty(self.local_decls, self.tcx).ty;
                if let Ok(layout) = self.tcx.layout_of(self.param_env.and(place_ty)) {
                    if layout.is_zst() && !layout.abi.is_uninhabited() {
                        if self.tcx.consider_optimizing(|| {
                            format!(
                                "InstCombine ZST - Place: {:?} SourceInfo: {:?}",
                                place, source_info
                            )
                        }) {
                            *kind = StatementKind::Nop;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Transform boolean comparisons into logical operations.
    fn combine_bool_cmp(&self, source_info: &SourceInfo, rvalue: &mut Rvalue<'tcx>) {
        match rvalue {
            Rvalue::BinaryOp(op @ (BinOp::Eq | BinOp::Ne), box (a, b)) => {
                let new = match (op, self.try_eval_bool(a), self.try_eval_bool(b)) {
                    // Transform "Eq(a, true)" ==> "a"
                    (BinOp::Eq, _, Some(true)) => Some(a.clone()),

                    // Transform "Ne(a, false)" ==> "a"
                    (BinOp::Ne, _, Some(false)) => Some(a.clone()),

                    // Transform "Eq(true, b)" ==> "b"
                    (BinOp::Eq, Some(true), _) => Some(b.clone()),

                    // Transform "Ne(false, b)" ==> "b"
                    (BinOp::Ne, Some(false), _) => Some(b.clone()),

                    // FIXME: Consider combining remaining comparisons into logical operations:
                    // Transform "Eq(false, b)" ==> "Not(b)"
                    // Transform "Ne(true, b)" ==> "Not(b)"
                    // Transform "Eq(a, false)" ==> "Not(a)"
                    // Transform "Ne(a, true)" ==> "Not(a)"
                    _ => None,
                };

                if let Some(new) = new {
                    if self.should_combine(source_info, rvalue) {
                        *rvalue = Rvalue::Use(new);
                    }
                }
            }

            _ => {}
        }
    }

    fn try_eval_bool(&self, a: &Operand<'_>) -> Option<bool> {
        let a = a.constant()?;
        if a.literal.ty.is_bool() { a.literal.val.try_to_bool() } else { None }
    }

    /// Transform "&(*a)" ==> "a".
    fn combine_ref_deref(&self, source_info: &SourceInfo, rvalue: &mut Rvalue<'tcx>) {
        if let Rvalue::Ref(_, _, place) = rvalue {
            if let Some((base, ProjectionElem::Deref)) = place.as_ref().last_projection() {
                if let ty::Ref(_, _, Mutability::Not) =
                    base.ty(self.local_decls, self.tcx).ty.kind()
                {
                    // The dereferenced place must have type `&_`, so that we don't copy `&mut _`.
                } else {
                    return;
                }

                if !self.should_combine(source_info, rvalue) {
                    return;
                }

                *rvalue = Rvalue::Use(Operand::Copy(Place {
                    local: base.local,
                    projection: self.tcx.intern_place_elems(base.projection),
                }));
            }
        }
    }

    /// Transform "Len([_; N])" ==> "N".
    fn combine_len(&self, source_info: &SourceInfo, rvalue: &mut Rvalue<'tcx>) {
        if let Rvalue::Len(ref place) = *rvalue {
            let place_ty = place.ty(self.local_decls, self.tcx).ty;
            if let ty::Array(_, len) = place_ty.kind() {
                if !self.should_combine(source_info, rvalue) {
                    return;
                }

                let constant = Constant { span: source_info.span, literal: len, user_ty: None };
                *rvalue = Rvalue::Use(Operand::Constant(box constant));
            }
        }
    }
}
