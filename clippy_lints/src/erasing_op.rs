// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use crate::consts::{constant_simple, Constant};
use crate::rustc::hir::*;
use crate::rustc::lint::{LateContext, LateLintPass, LintArray, LintPass};
use crate::rustc::{declare_tool_lint, lint_array};
use crate::syntax::source_map::Span;
use crate::utils::{in_macro, span_lint};

/// **What it does:** Checks for erasing operations, e.g. `x * 0`.
///
/// **Why is this bad?** The whole expression can be replaced by zero.
/// This is most likely not the intended outcome and should probably be
/// corrected
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// 0 / x; 0 * x; x & 0
/// ```
declare_clippy_lint! {
    pub ERASING_OP,
    correctness,
    "using erasing operations, e.g. `x * 0` or `y & 0`"
}

#[derive(Copy, Clone)]
pub struct ErasingOp;

impl LintPass for ErasingOp {
    fn get_lints(&self) -> LintArray {
        lint_array!(ERASING_OP)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for ErasingOp {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, e: &'tcx Expr) {
        if in_macro(e.span) {
            return;
        }
        if let ExprKind::Binary(ref cmp, ref left, ref right) = e.node {
            match cmp.node {
                BinOpKind::Mul | BinOpKind::BitAnd => {
                    check(cx, left, e.span);
                    check(cx, right, e.span);
                },
                BinOpKind::Div => check(cx, left, e.span),
                _ => (),
            }
        }
    }
}

fn check(cx: &LateContext<'_, '_>, e: &Expr, span: Span) {
    if let Some(Constant::Int(v)) = constant_simple(cx, cx.tables, e) {
        if v == 0 {
            span_lint(
                cx,
                ERASING_OP,
                span,
                "this operation will always return zero. This is likely not the intended outcome",
            );
        }
    }
}
