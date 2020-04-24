use crate::utils::{is_type_diagnostic_item, snippet_with_applicability, span_lint_and_sugg, walk_ptrs_ty};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, MatchSource};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::lint::in_external_macro;
use rustc_session::{declare_lint_pass, declare_tool_lint};

declare_clippy_lint! {
    /// **What it does:** Checks for `match vec[idx]` or `match vec[n..m]`.
    ///
    /// **Why is this bad?** This can panic at runtime.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust, no_run
    /// let arr = vec![0, 1, 2, 3];
    /// let idx = 1;
    ///
    /// // Bad
    /// match arr[idx] {
    ///     0 => println!("{}", 0),
    ///     1 => println!("{}", 3),
    ///     _ => {},
    /// }
    /// ```
    /// Use instead:
    /// ```rust, no_run
    /// let arr = vec![0, 1, 2, 3];
    /// let idx = 1;
    ///
    /// // Good
    /// match arr.get(idx) {
    ///     Some(0) => println!("{}", 0),
    ///     Some(1) => println!("{}", 3),
    ///     _ => {},
    /// }
    /// ```
    pub MATCH_VEC_ITEM,
    style,
    "match vector by indexing can panic"
}

declare_lint_pass!(MatchVecItem => [MATCH_VEC_ITEM]);

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for MatchVecItem {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if !in_external_macro(cx.sess(), expr.span);
            if let ExprKind::Match(ref match_expr, _, MatchSource::Normal) = expr.kind;
            if let Some(idx_expr) = is_vec_indexing(cx, match_expr);
            if let ExprKind::Index(vec, idx) = idx_expr.kind;

            then {
                let mut applicability = Applicability::MaybeIncorrect;
                span_lint_and_sugg(
                    cx,
                    MATCH_VEC_ITEM,
                    match_expr.span,
                    "indexing vector may panic. Consider using `get`",
                    "try this",
                    format!(
                        "{}.get({})",
                        snippet_with_applicability(cx, vec.span, "..", &mut applicability),
                        snippet_with_applicability(cx, idx.span, "..", &mut applicability)
                    ),
                    applicability
                );
            }
        }
    }
}

fn is_vec_indexing<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, expr: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    if_chain! {
        if let ExprKind::Index(ref array, _) = expr.kind;
        let ty = cx.tables.expr_ty(array);
        let ty = walk_ptrs_ty(ty);
        if is_type_diagnostic_item(cx, ty, sym!(vec_type));

        then {
            return Some(expr);
        }
    }

    None
}
