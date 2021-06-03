use clippy_utils::consts::{constant, Constant};
use clippy_utils::diagnostics::span_lint_and_note;
use if_chain::if_chain;
use rustc_ast::LitKind;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::source_map::Spanned;

use super::SUSPICIOUS_SPLITN;

pub(super) fn check(
    cx: &LateContext<'_>,
    method_name: &str,
    expr: &Expr<'_>,
    self_arg: &Expr<'_>,
    count_arg: &Expr<'_>,
) {
    if_chain! {
        if let Some((Constant::Int(count), _)) = constant(cx, cx.typeck_results(), count_arg);
        if count <= 1;
        if let Some(call_id) = cx.typeck_results().type_dependent_def_id(expr.hir_id);
        if let Some(impl_id) = cx.tcx.impl_of_method(call_id);
        let lang_items = cx.tcx.lang_items();
        if lang_items.slice_impl() == Some(impl_id) || lang_items.str_impl() == Some(impl_id);
        then {
            // Ignore empty slice and string literals when used with a literal count.
            if (matches!(self_arg.kind, ExprKind::Array([]))
                || matches!(self_arg.kind, ExprKind::Lit(Spanned { node: LitKind::Str(s, _), .. }) if s.is_empty())
            ) && matches!(count_arg.kind, ExprKind::Lit(_))
            {
                return;
            }

            let (msg, note_msg) = if count == 0 {
                (format!("`{}` called with `0` splits", method_name),
                "the resulting iterator will always return `None`")
            } else {
                (format!("`{}` called with `1` split", method_name),
                if lang_items.slice_impl() == Some(impl_id) {
                    "the resulting iterator will always return the entire slice followed by `None`"
                } else {
                    "the resulting iterator will always return the entire string followed by `None`"
                })
            };

            span_lint_and_note(
                cx,
                SUSPICIOUS_SPLITN,
                expr.span,
                &msg,
                None,
                note_msg,
            );
        }
    }
}
