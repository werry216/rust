use crate::utils::eager_or_lazy::is_lazyness_candidate;
use crate::utils::{contains_return, get_trait_def_id, last_path_segment, paths, span_lint_and_sugg};
use clippy_utils::source::{snippet, snippet_with_applicability, snippet_with_macro_callsite};
use clippy_utils::ty::{implements_trait, is_type_diagnostic_item, match_type};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::source_map::Span;
use rustc_span::symbol::sym;
use std::borrow::Cow;

use super::OR_FUN_CALL;

/// Checks for the `OR_FUN_CALL` lint.
#[allow(clippy::too_many_lines)]
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &hir::Expr<'_>,
    method_span: Span,
    name: &str,
    args: &'tcx [hir::Expr<'_>],
) {
    /// Checks for `unwrap_or(T::new())` or `unwrap_or(T::default())`.
    fn check_unwrap_or_default(
        cx: &LateContext<'_>,
        name: &str,
        fun: &hir::Expr<'_>,
        self_expr: &hir::Expr<'_>,
        arg: &hir::Expr<'_>,
        or_has_args: bool,
        span: Span,
    ) -> bool {
        if_chain! {
            if !or_has_args;
            if name == "unwrap_or";
            if let hir::ExprKind::Path(ref qpath) = fun.kind;
            let path = &*last_path_segment(qpath).ident.as_str();
            if ["default", "new"].contains(&path);
            let arg_ty = cx.typeck_results().expr_ty(arg);
            if let Some(default_trait_id) = get_trait_def_id(cx, &paths::DEFAULT_TRAIT);
            if implements_trait(cx, arg_ty, default_trait_id, &[]);

            then {
                let mut applicability = Applicability::MachineApplicable;
                span_lint_and_sugg(
                    cx,
                    OR_FUN_CALL,
                    span,
                    &format!("use of `{}` followed by a call to `{}`", name, path),
                    "try this",
                    format!(
                        "{}.unwrap_or_default()",
                        snippet_with_applicability(cx, self_expr.span, "..", &mut applicability)
                    ),
                    applicability,
                );

                true
            } else {
                false
            }
        }
    }

    /// Checks for `*or(foo())`.
    #[allow(clippy::too_many_arguments)]
    fn check_general_case<'tcx>(
        cx: &LateContext<'tcx>,
        name: &str,
        method_span: Span,
        self_expr: &hir::Expr<'_>,
        arg: &'tcx hir::Expr<'_>,
        span: Span,
        // None if lambda is required
        fun_span: Option<Span>,
    ) {
        // (path, fn_has_argument, methods, suffix)
        static KNOW_TYPES: [(&[&str], bool, &[&str], &str); 4] = [
            (&paths::BTREEMAP_ENTRY, false, &["or_insert"], "with"),
            (&paths::HASHMAP_ENTRY, false, &["or_insert"], "with"),
            (&paths::OPTION, false, &["map_or", "ok_or", "or", "unwrap_or"], "else"),
            (&paths::RESULT, true, &["or", "unwrap_or"], "else"),
        ];

        if let hir::ExprKind::MethodCall(ref path, _, ref args, _) = &arg.kind {
            if path.ident.as_str() == "len" {
                let ty = cx.typeck_results().expr_ty(&args[0]).peel_refs();

                match ty.kind() {
                    ty::Slice(_) | ty::Array(_, _) => return,
                    _ => (),
                }

                if is_type_diagnostic_item(cx, ty, sym::vec_type) {
                    return;
                }
            }
        }

        if_chain! {
            if KNOW_TYPES.iter().any(|k| k.2.contains(&name));

            if is_lazyness_candidate(cx, arg);
            if !contains_return(&arg);

            let self_ty = cx.typeck_results().expr_ty(self_expr);

            if let Some(&(_, fn_has_arguments, poss, suffix)) =
                KNOW_TYPES.iter().find(|&&i| match_type(cx, self_ty, i.0));

            if poss.contains(&name);

            then {
                let macro_expanded_snipped;
                let sugg: Cow<'_, str> = {
                    let (snippet_span, use_lambda) = match (fn_has_arguments, fun_span) {
                        (false, Some(fun_span)) => (fun_span, false),
                        _ => (arg.span, true),
                    };
                    let snippet = {
                        let not_macro_argument_snippet = snippet_with_macro_callsite(cx, snippet_span, "..");
                        if not_macro_argument_snippet == "vec![]" {
                            macro_expanded_snipped = snippet(cx, snippet_span, "..");
                            match macro_expanded_snipped.strip_prefix("$crate::vec::") {
                                Some(stripped) => Cow::from(stripped),
                                None => macro_expanded_snipped
                            }
                        }
                        else {
                            not_macro_argument_snippet
                        }
                    };

                    if use_lambda {
                        let l_arg = if fn_has_arguments { "_" } else { "" };
                        format!("|{}| {}", l_arg, snippet).into()
                    } else {
                        snippet
                    }
                };
                let span_replace_word = method_span.with_hi(span.hi());
                span_lint_and_sugg(
                    cx,
                    OR_FUN_CALL,
                    span_replace_word,
                    &format!("use of `{}` followed by a function call", name),
                    "try this",
                    format!("{}_{}({})", name, suffix, sugg),
                    Applicability::HasPlaceholders,
                );
            }
        }
    }

    if args.len() == 2 {
        match args[1].kind {
            hir::ExprKind::Call(ref fun, ref or_args) => {
                let or_has_args = !or_args.is_empty();
                if !check_unwrap_or_default(cx, name, fun, &args[0], &args[1], or_has_args, expr.span) {
                    let fun_span = if or_has_args { None } else { Some(fun.span) };
                    check_general_case(cx, name, method_span, &args[0], &args[1], expr.span, fun_span);
                }
            },
            hir::ExprKind::Index(..) | hir::ExprKind::MethodCall(..) => {
                check_general_case(cx, name, method_span, &args[0], &args[1], expr.span, None);
            },
            _ => {},
        }
    }
}
