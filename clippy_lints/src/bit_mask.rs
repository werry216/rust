use rustc::hir::*;
use rustc::hir::def::Def;
use rustc::lint::*;
use rustc_const_eval::lookup_const_by_id;
use syntax::ast::LitKind;
use syntax::codemap::Span;
use utils::span_lint;

/// **What it does:** Checks for incompatible bit masks in comparisons.
///
/// The formula for detecting if an expression of the type `_ <bit_op> m
/// <cmp_op> c` (where `<bit_op>` is one of {`&`, `|`} and `<cmp_op>` is one of
/// {`!=`, `>=`, `>`, `!=`, `>=`, `>`}) can be determined from the following
/// table:
///
/// |Comparison  |Bit Op|Example     |is always|Formula               |
/// |------------|------|------------|---------|----------------------|
/// |`==` or `!=`| `&`  |`x & 2 == 3`|`false`  |`c & m != c`          |
/// |`<`  or `>=`| `&`  |`x & 2 < 3` |`true`   |`m < c`               |
/// |`>`  or `<=`| `&`  |`x & 1 > 1` |`false`  |`m <= c`              |
/// |`==` or `!=`| `|`  |`x | 1 == 0`|`false`  |`c | m != c`          |
/// |`<`  or `>=`| `|`  |`x | 1 < 1` |`false`  |`m >= c`              |
/// |`<=` or `>` | `|`  |`x | 1 > 0` |`true`   |`m > c`               |
///
/// **Why is this bad?** If the bits that the comparison cares about are always
/// set to zero or one by the bit mask, the comparison is constant `true` or
/// `false` (depending on mask, compared value, and operators).
///
/// So the code is actively misleading, and the only reason someone would write
/// this intentionally is to win an underhanded Rust contest or create a
/// test-case for this lint.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// if (x & 1 == 2) { … }
/// ```
declare_lint! {
    pub BAD_BIT_MASK,
    Warn,
    "expressions of the form `_ & mask == select` that will only ever return `true` or `false`"
}

/// **What it does:** Checks for bit masks in comparisons which can be removed
/// without changing the outcome. The basic structure can be seen in the
/// following table:
///
/// |Comparison| Bit Op  |Example    |equals |
/// |----------|---------|-----------|-------|
/// |`>` / `<=`|`|` / `^`|`x | 2 > 3`|`x > 3`|
/// |`<` / `>=`|`|` / `^`|`x ^ 1 < 4`|`x < 4`|
///
/// **Why is this bad?** Not equally evil as [`bad_bit_mask`](#bad_bit_mask),
/// but still a bit misleading, because the bit mask is ineffective.
///
/// **Known problems:** False negatives: This lint will only match instances
/// where we have figured out the math (which is for a power-of-two compared
/// value). This means things like `x | 1 >= 7` (which would be better written
/// as `x >= 6`) will not be reported (but bit masks like this are fairly
/// uncommon).
///
/// **Example:**
/// ```rust
/// if (x | 1 > 3) { … }
/// ```
declare_lint! {
    pub INEFFECTIVE_BIT_MASK,
    Warn,
    "expressions where a bit mask will be rendered useless by a comparison, e.g. `(x | 1) > 2`"
}

#[derive(Copy,Clone)]
pub struct BitMask;

impl LintPass for BitMask {
    fn get_lints(&self) -> LintArray {
        lint_array!(BAD_BIT_MASK, INEFFECTIVE_BIT_MASK)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for BitMask {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, e: &'tcx Expr) {
        if let ExprBinary(ref cmp, ref left, ref right) = e.node {
            if cmp.node.is_comparison() {
                if let Some(cmp_opt) = fetch_int_literal(cx, right) {
                    check_compare(cx, left, cmp.node, cmp_opt, &e.span)
                } else if let Some(cmp_val) = fetch_int_literal(cx, left) {
                    check_compare(cx, right, invert_cmp(cmp.node), cmp_val, &e.span)
                }
            }
        }
    }
}

fn invert_cmp(cmp: BinOp_) -> BinOp_ {
    match cmp {
        BiEq => BiEq,
        BiNe => BiNe,
        BiLt => BiGt,
        BiGt => BiLt,
        BiLe => BiGe,
        BiGe => BiLe,
        _ => BiOr, // Dummy
    }
}


fn check_compare(cx: &LateContext, bit_op: &Expr, cmp_op: BinOp_, cmp_value: u64, span: &Span) {
    if let ExprBinary(ref op, ref left, ref right) = bit_op.node {
        if op.node != BiBitAnd && op.node != BiBitOr {
            return;
        }
        fetch_int_literal(cx, right)
            .or_else(|| fetch_int_literal(cx, left))
            .map_or((), |mask| check_bit_mask(cx, op.node, cmp_op, mask, cmp_value, span))
    }
}

fn check_bit_mask(cx: &LateContext, bit_op: BinOp_, cmp_op: BinOp_, mask_value: u64, cmp_value: u64, span: &Span) {
    match cmp_op {
        BiEq | BiNe => {
            match bit_op {
                BiBitAnd => {
                    if mask_value & cmp_value != cmp_value {
                        if cmp_value != 0 {
                            span_lint(cx,
                                      BAD_BIT_MASK,
                                      *span,
                                      &format!("incompatible bit mask: `_ & {}` can never be equal to `{}`",
                                               mask_value,
                                               cmp_value));
                        }
                    } else if mask_value == 0 {
                        span_lint(cx, BAD_BIT_MASK, *span, "&-masking with zero");
                    }
                }
                BiBitOr => {
                    if mask_value | cmp_value != cmp_value {
                        span_lint(cx,
                                  BAD_BIT_MASK,
                                  *span,
                                  &format!("incompatible bit mask: `_ | {}` can never be equal to `{}`",
                                           mask_value,
                                           cmp_value));
                    }
                }
                _ => (),
            }
        }
        BiLt | BiGe => {
            match bit_op {
                BiBitAnd => {
                    if mask_value < cmp_value {
                        span_lint(cx,
                                  BAD_BIT_MASK,
                                  *span,
                                  &format!("incompatible bit mask: `_ & {}` will always be lower than `{}`",
                                           mask_value,
                                           cmp_value));
                    } else if mask_value == 0 {
                        span_lint(cx, BAD_BIT_MASK, *span, "&-masking with zero");
                    }
                }
                BiBitOr => {
                    if mask_value >= cmp_value {
                        span_lint(cx,
                                  BAD_BIT_MASK,
                                  *span,
                                  &format!("incompatible bit mask: `_ | {}` will never be lower than `{}`",
                                           mask_value,
                                           cmp_value));
                    } else {
                        check_ineffective_lt(cx, *span, mask_value, cmp_value, "|");
                    }
                }
                BiBitXor => check_ineffective_lt(cx, *span, mask_value, cmp_value, "^"),
                _ => (),
            }
        }
        BiLe | BiGt => {
            match bit_op {
                BiBitAnd => {
                    if mask_value <= cmp_value {
                        span_lint(cx,
                                  BAD_BIT_MASK,
                                  *span,
                                  &format!("incompatible bit mask: `_ & {}` will never be higher than `{}`",
                                           mask_value,
                                           cmp_value));
                    } else if mask_value == 0 {
                        span_lint(cx, BAD_BIT_MASK, *span, "&-masking with zero");
                    }
                }
                BiBitOr => {
                    if mask_value > cmp_value {
                        span_lint(cx,
                                  BAD_BIT_MASK,
                                  *span,
                                  &format!("incompatible bit mask: `_ | {}` will always be higher than `{}`",
                                           mask_value,
                                           cmp_value));
                    } else {
                        check_ineffective_gt(cx, *span, mask_value, cmp_value, "|");
                    }
                }
                BiBitXor => check_ineffective_gt(cx, *span, mask_value, cmp_value, "^"),
                _ => (),
            }
        }
        _ => (),
    }
}

fn check_ineffective_lt(cx: &LateContext, span: Span, m: u64, c: u64, op: &str) {
    if c.is_power_of_two() && m < c {
        span_lint(cx,
                  INEFFECTIVE_BIT_MASK,
                  span,
                  &format!("ineffective bit mask: `x {} {}` compared to `{}`, is the same as x compared directly",
                           op,
                           m,
                           c));
    }
}

fn check_ineffective_gt(cx: &LateContext, span: Span, m: u64, c: u64, op: &str) {
    if (c + 1).is_power_of_two() && m <= c {
        span_lint(cx,
                  INEFFECTIVE_BIT_MASK,
                  span,
                  &format!("ineffective bit mask: `x {} {}` compared to `{}`, is the same as x compared directly",
                           op,
                           m,
                           c));
    }
}

fn fetch_int_literal(cx: &LateContext, lit: &Expr) -> Option<u64> {
    match lit.node {
        ExprLit(ref lit_ptr) => {
            if let LitKind::Int(value, _) = lit_ptr.node {
                Some(value) //TODO: Handle sign
            } else {
                None
            }
        }
        ExprPath(ref qpath) => {
            let def = cx.tcx.tables().qpath_def(qpath, lit.id);
            if let Def::Const(def_id) = def {
                lookup_const_by_id(cx.tcx, def_id, None).and_then(|(l, _ty)| fetch_int_literal(cx, l))
            } else {
                None
            }
        }
        _ => None,
    }
}
