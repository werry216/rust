use crate::utils::span_lint;
use rustc::lint::{EarlyContext, EarlyLintPass, LintArray, LintPass};
use rustc::{declare_lint_pass, declare_tool_lint};
use syntax::ast::*;
use syntax::source_map::Span;
use syntax::symbol::LocalInternedString;

declare_clippy_lint! {
    /// **What it does:** Checks for imports that remove "unsafe" from an item's
    /// name.
    ///
    /// **Why is this bad?** Renaming makes it less clear which traits and
    /// structures are unsafe.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust,ignore
    /// use std::cell::{UnsafeCell as TotallySafeCell};
    ///
    /// extern crate crossbeam;
    /// use crossbeam::{spawn_unsafe as spawn};
    /// ```
    pub UNSAFE_REMOVED_FROM_NAME,
    style,
    "`unsafe` removed from API names on import"
}

declare_lint_pass!(UnsafeNameRemoval => [UNSAFE_REMOVED_FROM_NAME]);

impl EarlyLintPass for UnsafeNameRemoval {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if let ItemKind::Use(ref use_tree) = item.kind {
            check_use_tree(use_tree, cx, item.span);
        }
    }
}

fn check_use_tree(use_tree: &UseTree, cx: &EarlyContext<'_>, span: Span) {
    match use_tree.kind {
        UseTreeKind::Simple(Some(new_name), ..) => {
            let old_name = use_tree
                .prefix
                .segments
                .last()
                .expect("use paths cannot be empty")
                .ident;
            unsafe_to_safe_check(old_name, new_name, cx, span);
        },
        UseTreeKind::Simple(None, ..) | UseTreeKind::Glob => {},
        UseTreeKind::Nested(ref nested_use_tree) => {
            for &(ref use_tree, _) in nested_use_tree {
                check_use_tree(use_tree, cx, span);
            }
        },
    }
}

fn unsafe_to_safe_check(old_name: Ident, new_name: Ident, cx: &EarlyContext<'_>, span: Span) {
    let old_str = old_name.name.as_str();
    let new_str = new_name.name.as_str();
    if contains_unsafe(&old_str) && !contains_unsafe(&new_str) {
        span_lint(
            cx,
            UNSAFE_REMOVED_FROM_NAME,
            span,
            &format!(
                "removed \"unsafe\" from the name of `{}` in use as `{}`",
                old_str, new_str
            ),
        );
    }
}

#[must_use]
fn contains_unsafe(name: &LocalInternedString) -> bool {
    name.contains("Unsafe") || name.contains("unsafe")
}
