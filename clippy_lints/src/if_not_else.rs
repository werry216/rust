// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


//! lint on if branches that could be swapped so no `!` operation is necessary
//! on the condition

use crate::rustc::lint::{EarlyContext, EarlyLintPass, LintArray, LintPass, in_external_macro, LintContext};
use crate::rustc::{declare_tool_lint, lint_array};
use crate::syntax::ast::*;

use crate::utils::span_help_and_lint;

/// **What it does:** Checks for usage of `!` or `!=` in an if condition with an
/// else branch.
///
/// **Why is this bad?** Negations reduce the readability of statements.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// if !v.is_empty() {
///     a()
/// } else {
///     b()
/// }
/// ```
///
/// Could be written:
///
/// ```rust
/// if v.is_empty() {
///     b()
/// } else {
///     a()
/// }
/// ```
declare_clippy_lint! {
    pub IF_NOT_ELSE,
    pedantic,
    "`if` branches that could be swapped so no negation operation is necessary on the condition"
}

pub struct IfNotElse;

impl LintPass for IfNotElse {
    fn get_lints(&self) -> LintArray {
        lint_array!(IF_NOT_ELSE)
    }
}

impl EarlyLintPass for IfNotElse {
    fn check_expr(&mut self, cx: &EarlyContext<'_>, item: &Expr) {
        if in_external_macro(cx.sess(), item.span) {
            return;
        }
        if let ExprKind::If(ref cond, _, Some(ref els)) = item.node {
            if let ExprKind::Block(..) = els.node {
                match cond.node {
                    ExprKind::Unary(UnOp::Not, _) => {
                        span_help_and_lint(
                            cx,
                            IF_NOT_ELSE,
                            item.span,
                            "Unnecessary boolean `not` operation",
                            "remove the `!` and swap the blocks of the if/else",
                        );
                    },
                    ExprKind::Binary(ref kind, _, _) if kind.node == BinOpKind::Ne => {
                        span_help_and_lint(
                            cx,
                            IF_NOT_ELSE,
                            item.span,
                            "Unnecessary `!=` operation",
                            "change to `==` and swap the blocks of the if/else",
                        );
                    },
                    _ => (),
                }
            }
        }
    }
}
