use rustc::{declare_lint, hir, lint, lint_array, ty};
use syntax::ast;
use crate::utils;

/// **What it does:** Checks for usage of the `offset` pointer method with a `usize` casted to an
/// `isize`.
///
/// **Why is this bad?** If we’re always increasing the pointer address, we can avoid the numeric
/// cast by using the `add` method instead.
///
/// **Known problems:** None
///
/// **Example:**
/// ```rust
/// let vec = vec![b'a', b'b', b'c'];
/// let ptr = vec.as_ptr();
/// let offset = 1_usize;
///
/// unsafe { ptr.offset(offset as isize); }
/// ```
///
/// Could be written:
///
/// ```rust
/// let vec = vec![b'a', b'b', b'c'];
/// let ptr = vec.as_ptr();
/// let offset = 1_usize;
///
/// unsafe { ptr.add(offset); }
/// ```
declare_clippy_lint! {
    pub PTR_OFFSET_WITH_CAST,
    complexity,
    "uneeded pointer offset cast"
}

#[derive(Copy, Clone, Debug)]
pub struct Pass;

impl lint::LintPass for Pass {
    fn get_lints(&self) -> lint::LintArray {
        lint_array!(PTR_OFFSET_WITH_CAST)
    }
}

impl<'a, 'tcx> lint::LateLintPass<'a, 'tcx> for Pass {
    fn check_expr(&mut self, cx: &lint::LateContext<'a, 'tcx>, expr: &'tcx hir::Expr) {
        // Check if the expressions is a ptr.offset method call
        let [receiver_expr, arg_expr] = match expr_as_ptr_offset_call(cx, expr) {
            Some(call_arg) => call_arg,
            None => return,
        };

        // Check if the argument to ptr.offset is a cast from usize
        let cast_lhs_expr = match expr_as_cast_from_usize(cx, arg_expr) {
            Some(cast_lhs_expr) => cast_lhs_expr,
            None => return,
        };

        utils::span_lint_and_sugg(
            cx,
            PTR_OFFSET_WITH_CAST,
            expr.span,
            "use of `offset` with a `usize` casted to an `isize`",
            "try",
            build_suggestion(cx, receiver_expr, cast_lhs_expr),
        );
    }
}

// If the given expression is a cast from a usize, return the lhs of the cast
fn expr_as_cast_from_usize<'a, 'tcx>(
    cx: &lint::LateContext<'a, 'tcx>,
    expr: &'tcx hir::Expr,
) -> Option<&'tcx hir::Expr> {
    if let hir::ExprKind::Cast(ref cast_lhs_expr, _) = expr.node {
        if is_expr_ty_usize(cx, &cast_lhs_expr) {
            return Some(cast_lhs_expr);
        }
    }
    None
}

// If the given expression is a ptr::offset method call, return the receiver and the arg of the
// method call.
fn expr_as_ptr_offset_call<'a, 'tcx>(
    cx: &lint::LateContext<'a, 'tcx>,
    expr: &'tcx hir::Expr,
) -> Option<[&'tcx hir::Expr; 2]> {
    if let hir::ExprKind::MethodCall(ref path_segment, _, ref args) = expr.node {
        if path_segment.ident.name == "offset" && is_expr_ty_raw_ptr(cx, &args[0]) {
            return Some([&args[0], &args[1]]);
        }
    }
    None
}

// Is the type of the expression a usize?
fn is_expr_ty_usize<'a, 'tcx>(
    cx: &lint::LateContext<'a, 'tcx>,
    expr: &hir::Expr,
) -> bool {
    cx.tables.expr_ty(expr).sty == ty::TyKind::Uint(ast::UintTy::Usize)
}

// Is the type of the expression a raw pointer?
fn is_expr_ty_raw_ptr<'a, 'tcx>(
    cx: &lint::LateContext<'a, 'tcx>,
    expr: &hir::Expr,
) -> bool {
    if let ty::RawPtr(..) = cx.tables.expr_ty(expr).sty {
        true
    } else {
        false
    }
}

fn build_suggestion<'a, 'tcx>(
    cx: &lint::LateContext<'a, 'tcx>,
    receiver_expr: &hir::Expr,
    cast_lhs_expr: &hir::Expr,
) -> String {
    match (
        utils::snippet_opt(cx, receiver_expr.span),
        utils::snippet_opt(cx, cast_lhs_expr.span)
    ) {
        (Some(receiver), Some(cast_lhs)) => format!("{}.add({})", receiver, cast_lhs),
        _ => String::new(),
    }
}
