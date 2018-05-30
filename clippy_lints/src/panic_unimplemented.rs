use rustc::hir::*;
use rustc::lint::*;
use syntax::ast::LitKind;
use syntax::ptr::P;
use syntax::ext::quote::rt::Span;
use crate::utils::{is_direct_expn_of, is_expn_of, match_def_path, opt_def_id, paths, resolve_node, span_lint};

/// **What it does:** Checks for missing parameters in `panic!`.
///
/// **Why is this bad?** Contrary to the `format!` family of macros, there are
/// two forms of `panic!`: if there are no parameters given, the first argument
/// is not a format string and used literally. So while `format!("{}")` will
/// fail to compile, `panic!("{}")` will not.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// panic!("This `panic!` is probably missing a parameter there: {}");
/// ```
declare_clippy_lint! {
    pub PANIC_PARAMS,
    style,
    "missing parameters in `panic!` calls"
}

/// **What it does:** Checks for usage of `unimplemented!`.
///
/// **Why is this bad?** This macro should not be present in production code
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// unimplemented!();
/// ```
declare_clippy_lint! {
    pub UNIMPLEMENTED,
    restriction,
    "`unimplemented!` should not be present in production code"
}

#[allow(missing_copy_implementations)]
pub struct Pass;

impl LintPass for Pass {
    fn get_lints(&self) -> LintArray {
        lint_array!(PANIC_PARAMS, UNIMPLEMENTED)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for Pass {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx Expr) {
        if_chain! {
            if let ExprBlock(ref block, _) = expr.node;
            if let Some(ref ex) = block.expr;
            if let ExprCall(ref fun, ref params) = ex.node;
            if let ExprPath(ref qpath) = fun.node;
            if let Some(fun_def_id) = opt_def_id(resolve_node(cx, qpath, fun.hir_id));
            if match_def_path(cx.tcx, fun_def_id, &paths::BEGIN_PANIC);
            if params.len() == 2;
            then {
                if is_expn_of(expr.span, "unimplemented").is_some() {
                    let span = get_outer_span(expr);
                    span_lint(cx, UNIMPLEMENTED, span,
                              "`unimplemented` should not be present in production code");
                } else {
                    match_panic(params, expr, cx);
                }
            }
        }
    }
}

fn get_outer_span(expr: &Expr) -> Span {
    if_chain! {
        if let Some(first) = expr.span.ctxt().outer().expn_info();
        if let Some(second) = first.call_site.ctxt().outer().expn_info();
        then {
            second.call_site
        } else {
            expr.span
        }
    }
}

fn match_panic(params: &P<[Expr]>, expr: &Expr, cx: &LateContext) {
    if_chain! {
        if let ExprLit(ref lit) = params[0].node;
        if is_direct_expn_of(expr.span, "panic").is_some();
        if let LitKind::Str(ref string, _) = lit.node;
        let string = string.as_str().replace("{{", "").replace("}}", "");
        if let Some(par) = string.find('{');
        if string[par..].contains('}');
        if params[0].span.source_callee().is_none();
        if params[0].span.lo() != params[0].span.hi();
        then {
            span_lint(cx, PANIC_PARAMS, params[0].span,
                      "you probably are missing some parameter in your format string");
        }
    }
}
