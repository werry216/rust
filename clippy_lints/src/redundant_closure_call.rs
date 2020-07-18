use crate::utils::{snippet_with_applicability, span_lint, span_lint_and_then};
use if_chain::if_chain;
use rustc_ast::ast;
use rustc_ast::visit as ast_visit;
use rustc_ast::visit::Visitor as AstVisitor;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_hir::intravisit as hir_visit;
use rustc_hir::intravisit::Visitor as HirVisitor;
use rustc_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass, LintContext};
use rustc_middle::hir::map::Map;
use rustc_middle::lint::in_external_macro;
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::symbol::Ident;

declare_clippy_lint! {
    /// **What it does:** Detects closures called in the same expression where they
    /// are defined.
    ///
    /// **Why is this bad?** It is unnecessarily adding to the expression's
    /// complexity.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// // Bad
    /// let a = (|| 42)()
    ///
    /// // Good
    /// let a = 42
    /// ```
    pub REDUNDANT_CLOSURE_CALL,
    complexity,
    "throwaway closures called in the expression they are defined"
}

declare_lint_pass!(RedundantClosureCall => [REDUNDANT_CLOSURE_CALL]);

// Used to find `return` statements or equivalents e.g., `?`
struct ReturnVisitor {
    found_return: bool,
}

impl ReturnVisitor {
    #[must_use]
    fn new() -> Self {
        Self { found_return: false }
    }
}

impl<'ast> ast_visit::Visitor<'ast> for ReturnVisitor {
    fn visit_expr(&mut self, ex: &'ast ast::Expr) {
        if let ast::ExprKind::Ret(_) = ex.kind {
            self.found_return = true;
        } else if let ast::ExprKind::Try(_) = ex.kind {
            self.found_return = true;
        }

        ast_visit::walk_expr(self, ex)
    }
}

impl EarlyLintPass for RedundantClosureCall {
    fn check_expr(&mut self, cx: &EarlyContext<'_>, expr: &ast::Expr) {
        if in_external_macro(cx.sess(), expr.span) {
            return;
        }
        if_chain! {
            if let ast::ExprKind::Call(ref paren, _) = expr.kind;
            if let ast::ExprKind::Paren(ref closure) = paren.kind;
            if let ast::ExprKind::Closure(_, _, _, ref decl, ref block, _) = closure.kind;
            then {
                let mut visitor = ReturnVisitor::new();
                visitor.visit_expr(block);
                if !visitor.found_return {
                    span_lint_and_then(
                        cx,
                        REDUNDANT_CLOSURE_CALL,
                        expr.span,
                        "Try not to call a closure in the expression where it is declared.",
                        |diag| {
                            if decl.inputs.is_empty() {
                                let mut app = Applicability::MachineApplicable;
                                let hint =
                                    snippet_with_applicability(cx, block.span, "..", &mut app).into_owned();
                                diag.span_suggestion(expr.span, "Try doing something like: ", hint, app);
                            }
                        },
                    );
                }
            }
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for RedundantClosureCall {
    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx hir::Block<'_>) {
        fn count_closure_usage<'tcx>(block: &'tcx hir::Block<'_>, ident: &'tcx Ident) -> usize {
            struct ClosureUsageCount<'tcx> {
                ident: &'tcx Ident,
                count: usize,
            };
            impl<'tcx> hir_visit::Visitor<'tcx> for ClosureUsageCount<'tcx> {
                type Map = Map<'tcx>;

                fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
                    if_chain! {
                        if let hir::ExprKind::Call(ref closure, _) = expr.kind;
                        if let hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) = closure.kind;
                        if self.ident == &path.segments[0].ident;
                        then {
                            self.count += 1;
                        }
                    }
                    hir_visit::walk_expr(self, expr);
                }

                fn nested_visit_map(&mut self) -> hir_visit::NestedVisitorMap<Self::Map> {
                    hir_visit::NestedVisitorMap::None
                }
            };
            let mut closure_usage_count = ClosureUsageCount { ident, count: 0 };
            closure_usage_count.visit_block(block);
            closure_usage_count.count
        }

        for w in block.stmts.windows(2) {
            if_chain! {
                if let hir::StmtKind::Local(ref local) = w[0].kind;
                if let Option::Some(ref t) = local.init;
                if let hir::ExprKind::Closure(..) = t.kind;
                if let hir::PatKind::Binding(_, _, ident, _) = local.pat.kind;
                if let hir::StmtKind::Semi(ref second) = w[1].kind;
                if let hir::ExprKind::Assign(_, ref call, _) = second.kind;
                if let hir::ExprKind::Call(ref closure, _) = call.kind;
                if let hir::ExprKind::Path(hir::QPath::Resolved(_, ref path)) = closure.kind;
                if ident == path.segments[0].ident;
                if  count_closure_usage(block, &ident) == 1;
                then {
                    span_lint(
                        cx,
                        REDUNDANT_CLOSURE_CALL,
                        second.span,
                        "Closure called just once immediately after it was declared",
                    );
                }
            }
        }
    }
}
