use crate::utils;
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::lint::in_external_macro;
use rustc_session::{declare_lint_pass, declare_tool_lint};

declare_clippy_lint! {
    /// **What it does:** Checks for if-else that could be written to `bool::then`.
    ///
    /// **Why is this bad?** Looks a little redundant. Using `bool::then` helps it have less lines of code.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// # let v = vec![0];
    /// let a = if v.is_empty() {
    ///     println!("true!");
    ///     Some(42)
    /// } else {
    ///     None
    /// };
    /// ```
    ///
    /// Could be written:
    ///
    /// ```rust
    /// # let v = vec![0];
    /// let a = v.is_empty().then(|| {
    ///     println!("true!");
    ///     42
    /// });
    /// ```
    pub IF_THEN_SOME_ELSE_NONE,
    restriction,
    "Finds if-else that could be written using `bool::then`"
}

declare_lint_pass!(IfThenSomeElseNone => [IF_THEN_SOME_ELSE_NONE]);

impl LateLintPass<'_> for IfThenSomeElseNone {
    fn check_expr(&mut self, cx: &LateContext<'_>, expr: &'tcx Expr<'_>) {
        if in_external_macro(cx.sess(), expr.span) {
            return;
        }

        // We only care about the top-most `if` in the chain
        if utils::parent_node_is_if_expr(expr, cx) {
            return;
        }

        if_chain! {
            if let ExprKind::If(ref cond, ref then, Some(ref els)) = expr.kind;
            if let ExprKind::Block(ref then_block, _) = then.kind;
            if let Some(ref then_expr) = then_block.expr;
            if let ExprKind::Call(ref then_call, [then_arg]) = then_expr.kind;
            if let ExprKind::Path(ref then_call_qpath) = then_call.kind;
            if utils::match_qpath(then_call_qpath, &utils::paths::OPTION_SOME);
            if let ExprKind::Block(ref els_block, _) = els.kind;
            if els_block.stmts.is_empty();
            if let Some(ref els_expr) = els_block.expr;
            if let ExprKind::Path(ref els_call_qpath) = els_expr.kind;
            if utils::match_qpath(els_call_qpath, &utils::paths::OPTION_NONE);
            then {
                let mut applicability = Applicability::MachineApplicable;
                let cond_snip = utils::snippet_with_applicability(cx, cond.span, "[condition]", &mut applicability);
                let arg_snip = utils::snippet_with_applicability(cx, then_arg.span, "", &mut applicability);
                let sugg = format!(
                    "{}.then(|| {{ /* snippet */ {} }})",
                    cond_snip,
                    arg_snip,
                );
                utils::span_lint_and_sugg(
                    cx,
                    IF_THEN_SOME_ELSE_NONE,
                    expr.span,
                    "this could be simplified with `bool::then`",
                    "try this",
                    sugg,
                    applicability,
                );
            }
        }
    }
}
