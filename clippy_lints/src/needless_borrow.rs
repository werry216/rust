//! Checks for needless address of operations (`&`)
//!
//! This lint is **warn** by default

use rustc::lint::*;
use rustc::hir::{ExprAddrOf, Expr, MutImmutable, Pat, PatKind, BindingMode};
use rustc::ty;
use utils::{span_lint, in_macro};

/// **What it does:** Checks for address of operations (`&`) that are going to
/// be dereferenced immediately by the compiler.
///
/// **Why is this bad?** Suggests that the receiver of the expression borrows
/// the expression.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// let x: &i32 = &&&&&&5;
/// ```
declare_lint! {
    pub NEEDLESS_BORROW,
    Warn,
    "taking a reference that is going to be automatically dereferenced"
}

#[derive(Copy,Clone)]
pub struct NeedlessBorrow;

impl LintPass for NeedlessBorrow {
    fn get_lints(&self) -> LintArray {
        lint_array!(NEEDLESS_BORROW)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for NeedlessBorrow {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, e: &'tcx Expr) {
        if in_macro(cx, e.span) {
            return;
        }
        if let ExprAddrOf(MutImmutable, ref inner) = e.node {
            if let ty::TyRef(..) = cx.tables.expr_ty(inner).sty {
                if let Some(&ty::adjustment::Adjust::DerefRef { autoderefs, autoref, .. }) =
                    cx.tables.adjustments.get(&e.id).map(|a| &a.kind) {
                    if autoderefs > 1 && autoref.is_some() {
                        span_lint(cx,
                                  NEEDLESS_BORROW,
                                  e.span,
                                  "this expression borrows a reference that is immediately dereferenced by the \
                                   compiler");
                    }
                }
            }
        }
    }
    fn check_pat(&mut self, cx: &LateContext<'a, 'tcx>, pat: &'tcx Pat) {
        if in_macro(cx, pat.span) {
            return;
        }
        if_let_chain! {[
            let PatKind::Binding(BindingMode::BindByRef(MutImmutable), _, _, _) = pat.node,
            let ty::TyRef(_, ref tam) = cx.tables.pat_ty(pat).sty,
            tam.mutbl == MutImmutable,
            let ty::TyRef(_, ref tam) = tam.ty.sty,
            // only lint immutable refs, because borrowed `&mut T` cannot be moved out
            tam.mutbl == MutImmutable,
        ], {
            span_lint(cx, NEEDLESS_BORROW, pat.span, "this pattern creates a reference to a reference")
        }}
    }
}
