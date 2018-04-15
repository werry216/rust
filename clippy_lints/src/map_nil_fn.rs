use rustc::hir;
use rustc::lint::*;
use rustc::ty;
use std::borrow::Cow;
use utils::{in_macro, iter_input_pats, match_type, method_chain_args, snippet, span_lint_and_then};
use utils::paths;

#[derive(Clone)]
pub struct Pass;

/// **What it does:** Checks for usage of `Option.map(f)` where f is a nil
/// function or closure
///
/// **Why is this bad?** Readability, this can be written more clearly with
/// an if statement
///
/// **Known problems:** Closures with multiple statements are not handled
///
/// **Example:**
/// ```rust
/// let x : Option<&str> = do_stuff();
/// x.map(log_err_msg);
/// x.map(|msg| log_err_msg(format_msg(msg)))
/// ```
/// The correct use would be:
/// ```rust
/// let x : Option<&str> = do_stuff();
/// if let Some(msg) = x {
///     log_err_msg(msg)
/// }
/// if let Some(msg) = x {
///     log_err_msg(format_msg(msg))
/// }
/// ```
declare_lint! {
    pub OPTION_MAP_NIL_FN,
    Allow,
    "using `Option.map(f)`, where f is a nil function or closure"
}


impl LintPass for Pass {
    fn get_lints(&self) -> LintArray {
        lint_array!(OPTION_MAP_NIL_FN)
    }
}

fn is_nil_type(ty: ty::Ty) -> bool {
    match ty.sty {
        ty::TyTuple(slice) => slice.is_empty(),
        ty::TyNever => true,
        _ => false,
    }
}

fn is_nil_function(cx: &LateContext, expr: &hir::Expr) -> bool {
    let ty = cx.tables.expr_ty(expr);

    if let ty::TyFnDef(_, _, bare) = ty.sty {
        if let Some(fn_type) = cx.tcx.no_late_bound_regions(&bare.sig) {
            return is_nil_type(fn_type.output());
        }
    }
    false
}

fn is_nil_expression(cx: &LateContext, expr: &hir::Expr) -> bool {
    is_nil_type(cx.tables.expr_ty(expr))
}

// The expression inside a closure may or may not have surrounding braces and
// semicolons, which causes problems when generating a suggestion. Given an
// expression that evaluates to '()' or '!', recursively remove useless braces
// and semi-colons until is suitable for including in the suggestion template
fn reduce_nil_expression<'a>(cx: &LateContext, expr: &'a hir::Expr) -> Option<Cow<'a, str>> {
    if !is_nil_expression(cx, expr) {
        return None;
    }

    match expr.node {
        hir::ExprCall(_, _) |
        hir::ExprMethodCall(_, _, _) => {
            // Calls can't be reduced any more
            Some(snippet(cx, expr.span, "_"))
        },
        hir::ExprBlock(ref block) => {
            match (&block.stmts[..], block.expr.as_ref()) {
                (&[], Some(inner_expr)) => {
                    // Reduce `{ X }` to `X`
                    reduce_nil_expression(cx, inner_expr)
                },
                (&[ref inner_stmt], None) => {
                    // Reduce `{ X; }` to `X` or `X;`
                    match inner_stmt.node {
                        hir::StmtDecl(ref d, _) => Some(snippet(cx, d.span, "_")),
                        hir::StmtExpr(ref e, _) => Some(snippet(cx, e.span, "_")),
                        hir::StmtSemi(ref e, _) => {
                            if is_nil_expression(cx, e) {
                                // `X` returns nil so we can strip the
                                // semicolon and reduce further
                                reduce_nil_expression(cx, e)
                            } else {
                                // `X` doesn't return nil so it needs a
                                // trailing semicolon
                                Some(snippet(cx, inner_stmt.span, "_"))
                            }
                        },
                    }
                },
                _ => None,
            }
        },
        _ => None,
    }
}

fn reduce_nil_closure<'a, 'tcx>(
    cx: &LateContext<'a, 'tcx>,
    expr: &'a hir::Expr
) -> Option<(Cow<'a, str>, Cow<'a, str>)> {
    if let hir::ExprClosure(_, ref decl, inner_expr_id, _) = expr.node {
        let body = cx.tcx.map.body(inner_expr_id);

        if_let_chain! {[
            decl.inputs.len() == 1,
            let Some(binding) = iter_input_pats(&decl, body).next(),
            let Some(expr_snippet) = reduce_nil_expression(cx, &body.value),
        ], {
            let binding_snippet = snippet(cx, binding.pat.span, "_");
            return Some((binding_snippet, expr_snippet));
        }}
    }
    None
}

fn lint_map_nil_fn(cx: &LateContext, stmt: &hir::Stmt, expr: &hir::Expr, map_args: &[hir::Expr]) {
    let var_arg = &map_args[0];
    let fn_arg = &map_args[1];

    if !match_type(cx, cx.tables.expr_ty(var_arg), &paths::OPTION) {
        return;
    }

    if is_nil_function(cx, fn_arg) {
        let msg = "called `map(f)` on an Option value where `f` is a nil function";
        let suggestion = format!("if let Some(...) = {0} {{ {1}(...) }}",
                                 snippet(cx, var_arg.span, "_"),
                                 snippet(cx, fn_arg.span, "_"));

        span_lint_and_then(cx,
                           OPTION_MAP_NIL_FN,
                           expr.span,
                           msg,
                           |db| { db.span_suggestion(stmt.span, "try this", suggestion); });
    } else if let Some((binding_snippet, expr_snippet)) = reduce_nil_closure(cx, fn_arg) {
        let msg = "called `map(f)` on an Option value where `f` is a nil closure";
        let suggestion = format!("if let Some({0}) = {1} {{ {2} }}",
                                 binding_snippet,
                                 snippet(cx, var_arg.span, "_"),
                                 expr_snippet);

        span_lint_and_then(cx,
                           OPTION_MAP_NIL_FN,
                           expr.span,
                           msg,
                           |db| { db.span_suggestion(stmt.span, "try this", suggestion); });
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for Pass {
    fn check_stmt(&mut self, cx: &LateContext, stmt: &hir::Stmt) {
        if in_macro(cx, stmt.span) {
            return;
        }

        if let hir::StmtSemi(ref expr, _) = stmt.node {
            if let hir::ExprMethodCall(_, _, _) = expr.node {
                if let Some(arglists) = method_chain_args(expr, &["map"]) {
                    lint_map_nil_fn(cx, stmt, expr, arglists[0]);
                }
            }
        }
    }
}
