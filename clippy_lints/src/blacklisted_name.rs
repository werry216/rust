use crate::utils::span_lint;
use rustc::hir::*;
use rustc::impl_lint_pass;
use rustc::lint::{LateContext, LateLintPass, LintArray, LintPass};
use rustc_data_structures::fx::FxHashSet;
use rustc_session::declare_tool_lint;

declare_clippy_lint! {
    /// **What it does:** Checks for usage of blacklisted names for variables, such
    /// as `foo`.
    ///
    /// **Why is this bad?** These names are usually placeholder names and should be
    /// avoided.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// let foo = 3.14;
    /// ```
    pub BLACKLISTED_NAME,
    style,
    "usage of a blacklisted/placeholder name"
}

#[derive(Clone, Debug)]
pub struct BlacklistedName {
    blacklist: FxHashSet<String>,
}

impl BlacklistedName {
    pub fn new(blacklist: FxHashSet<String>) -> Self {
        Self { blacklist }
    }
}

impl_lint_pass!(BlacklistedName => [BLACKLISTED_NAME]);

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for BlacklistedName {
    fn check_pat(&mut self, cx: &LateContext<'a, 'tcx>, pat: &'tcx Pat<'_>) {
        if let PatKind::Binding(.., ident, _) = pat.kind {
            if self.blacklist.contains(&ident.name.to_string()) {
                span_lint(
                    cx,
                    BLACKLISTED_NAME,
                    ident.span,
                    &format!("use of a blacklisted/placeholder name `{}`", ident.name),
                );
            }
        }
    }
}
