use rustc::lint::*;
use rustc::hir::*;
use syntax::codemap::Spanned;
use utils::{is_integer_literal, match_type, paths, snippet, span_lint};
use utils::higher;

/// **What it does:** Checks for iterating over ranges with a `.step_by(0)`,
/// which never terminates.
///
/// **Why is this bad?** This very much looks like an oversight, since with
/// `loop { .. }` there is an obvious better way to endlessly loop.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// for x in (5..5).step_by(0) { .. }
/// ```
declare_lint! {
    pub RANGE_STEP_BY_ZERO,
    Warn,
    "using `Range::step_by(0)`, which produces an infinite iterator"
}
/// **What it does:** Checks for zipping a collection with the range of `0.._.len()`.
///
/// **Why is this bad?** The code is better expressed with `.enumerate()`.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// x.iter().zip(0..x.len())
/// ```
declare_lint! {
    pub RANGE_ZIP_WITH_LEN,
    Warn,
    "zipping iterator with a range when `enumerate()` would do"
}

#[derive(Copy,Clone)]
pub struct StepByZero;

impl LintPass for StepByZero {
    fn get_lints(&self) -> LintArray {
        lint_array!(RANGE_STEP_BY_ZERO, RANGE_ZIP_WITH_LEN)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for StepByZero {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx Expr) {
        if let ExprMethodCall(Spanned { node: ref name, .. }, _, ref args) = expr.node {
            let name = &*name.as_str();

            // Range with step_by(0).
            if name == "step_by" && args.len() == 2 && has_step_by(cx, &args[0]) &&
               is_integer_literal(&args[1], 0) {
                span_lint(cx,
                          RANGE_STEP_BY_ZERO,
                          expr.span,
                          "Range::step_by(0) produces an infinite iterator. Consider using `std::iter::repeat()` \
                           instead");
            } else if name == "zip" && args.len() == 2 {
                let iter = &args[0].node;
                let zip_arg = &args[1];
                if_let_chain! {[
                    // .iter() call
                    let ExprMethodCall( Spanned { node: ref iter_name, .. }, _, ref iter_args ) = *iter,
                    &*iter_name.as_str() == "iter",
                    // range expression in .zip() call: 0..x.len()
                    let Some(higher::Range { start: Some(ref start), end: Some(ref end), .. }) = higher::range(zip_arg),
                    is_integer_literal(start, 0),
                    // .len() call
                    let ExprMethodCall(Spanned { node: ref len_name, .. }, _, ref len_args) = end.node,
                    &*len_name.as_str() == "len" && len_args.len() == 1,
                    // .iter() and .len() called on same Path
                    let ExprPath(QPath::Resolved(_, ref iter_path)) = iter_args[0].node,
                    let ExprPath(QPath::Resolved(_, ref len_path)) = len_args[0].node,
                    iter_path.segments == len_path.segments
                 ], {
                     span_lint(cx,
                               RANGE_ZIP_WITH_LEN,
                               expr.span,
                               &format!("It is more idiomatic to use {}.iter().enumerate()",
                                        snippet(cx, iter_args[0].span, "_")));
                }}
            }
        }
    }
}

fn has_step_by(cx: &LateContext, expr: &Expr) -> bool {
    // No need for walk_ptrs_ty here because step_by moves self, so it
    // can't be called on a borrowed range.
    let ty = cx.tcx.tables().expr_ty(expr);

    // Note: `RangeTo`, `RangeToInclusive` and `RangeFull` don't have step_by
    match_type(cx, ty, &paths::RANGE)
        || match_type(cx, ty, &paths::RANGE_FROM)
        || match_type(cx, ty, &paths::RANGE_INCLUSIVE)
}
