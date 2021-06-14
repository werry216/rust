use rustc_ast::Mutability;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_middle::ty::Ty;
use rustc_session::lint::builtin::FUTURE_PRELUDE_COLLISION;
use rustc_span::symbol::{sym, Ident};
use rustc_span::Span;

use crate::check::{
    method::probe::{self, Pick},
    FnCtxt,
};

impl<'a, 'tcx> FnCtxt<'a, 'tcx> {
    pub(super) fn lint_dot_call_from_2018(
        &self,
        self_ty: Ty<'tcx>,
        segment: &hir::PathSegment<'_>,
        span: Span,
        call_expr: &'tcx hir::Expr<'tcx>,
        self_expr: &'tcx hir::Expr<'tcx>,
        pick: &Pick<'tcx>,
    ) {
        debug!(
            "lookup(method_name={}, self_ty={:?}, call_expr={:?}, self_expr={:?})",
            segment.ident, self_ty, call_expr, self_expr
        );

        // Rust 2021 and later is already using the new prelude
        if span.rust_2021() {
            return;
        }

        // These are the method names that were added to prelude in Rust 2021
        if !matches!(segment.ident.name, sym::try_into) {
            return;
        }

        // No need to lint if method came from std/core, as that will now be in the prelude
        if matches!(self.tcx.crate_name(pick.item.def_id.krate), sym::std | sym::core) {
            return;
        }

        self.tcx.struct_span_lint_hir(
            FUTURE_PRELUDE_COLLISION,
            call_expr.hir_id,
            call_expr.span,
            |lint| {
                let sp = call_expr.span;
                let trait_name = self.tcx.def_path_str(pick.item.container.id());

                let mut lint = lint.build(&format!(
                    "trait method `{}` will become ambiguous in Rust 2021",
                    segment.ident.name
                ));

                if let Ok(self_expr) = self.sess().source_map().span_to_snippet(self_expr.span) {
                    let derefs = "*".repeat(pick.autoderefs);

                    let autoref = match pick.autoref_or_ptr_adjustment {
                        Some(probe::AutorefOrPtrAdjustment::Autoref {
                            mutbl: Mutability::Mut,
                            ..
                        }) => "&mut ",
                        Some(probe::AutorefOrPtrAdjustment::Autoref {
                            mutbl: Mutability::Not,
                            ..
                        }) => "&",
                        Some(probe::AutorefOrPtrAdjustment::ToConstPtr) | None => "",
                    };
                    let self_adjusted = if let Some(probe::AutorefOrPtrAdjustment::ToConstPtr) =
                        pick.autoref_or_ptr_adjustment
                    {
                        format!("{}{} as *const _", derefs, self_expr)
                    } else {
                        format!("{}{}{}", autoref, derefs, self_expr)
                    };
                    lint.span_suggestion(
                        sp,
                        "disambiguate the associated function",
                        format!("{}::{}({})", trait_name, segment.ident.name, self_adjusted,),
                        Applicability::MachineApplicable,
                    );
                } else {
                    lint.span_help(
                        sp,
                        &format!(
                            "disambiguate the associated function with `{}::{}(...)`",
                            trait_name, segment.ident,
                        ),
                    );
                }

                lint.emit();
            },
        );
    }

    pub(super) fn lint_fully_qualified_call_from_2018(
        &self,
        span: Span,
        method_name: Ident,
        self_ty: Ty<'tcx>,
        self_ty_span: Span,
        expr_id: hir::HirId,
        pick: &Pick<'tcx>,
    ) {
        // Rust 2021 and later is already using the new prelude
        if span.rust_2021() {
            return;
        }

        // These are the fully qualified methods added to prelude in Rust 2021
        if !matches!(method_name.name, sym::try_into | sym::try_from | sym::from_iter) {
            return;
        }

        // No need to lint if method came from std/core, as that will now be in the prelude
        if matches!(self.tcx.crate_name(pick.item.def_id.krate), sym::std | sym::core) {
            return;
        }

        // No need to lint if this is an inherent method called on a specific type, like `Vec::foo(...)`,
        // since such methods take precedence over trait methods.
        if matches!(pick.kind, probe::PickKind::InherentImplPick) {
            return;
        }

        self.tcx.struct_span_lint_hir(FUTURE_PRELUDE_COLLISION, expr_id, span, |lint| {
            // "type" refers to either a type or, more likely, a trait from which
            // the associated function or method is from.
            let type_name = self.tcx.def_path_str(pick.item.container.id());
            let type_generics = self.tcx.generics_of(pick.item.container.id());

            let parameter_count = type_generics.count() - (type_generics.has_self as usize);
            let trait_name = if parameter_count == 0 {
                type_name
            } else {
                format!(
                    "{}<{}>",
                    type_name,
                    std::iter::repeat("_").take(parameter_count).collect::<Vec<_>>().join(", ")
                )
            };

            let mut lint = lint.build(&format!(
                "trait-associated function `{}` will become ambiguous in Rust 2021",
                method_name.name
            ));

            let self_ty = self
                .sess()
                .source_map()
                .span_to_snippet(self_ty_span)
                .unwrap_or_else(|_| self_ty.to_string());

            lint.span_suggestion(
                span,
                "disambiguate the associated function",
                format!("<{} as {}>::{}", self_ty, trait_name, method_name.name,),
                Applicability::MachineApplicable,
            );

            lint.emit();
        });
    }
}
