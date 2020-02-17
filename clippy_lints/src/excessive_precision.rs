use crate::utils::span_lint_and_sugg;
use crate::utils::sugg::format_numeric_literal;
use if_chain::if_chain;
use rustc::ty;
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use std::{f32, f64, fmt};
use syntax::ast::*;

declare_clippy_lint! {
    /// **What it does:** Checks for float literals with a precision greater
    /// than that supported by the underlying type.
    ///
    /// **Why is this bad?** Rust will silently lose precision during conversion
    /// to a float.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust
    /// // Bad
    /// let a: f32 = 0.123_456_789_9; // 0.123_456_789
    /// let b: f32 = 16_777_217.0; // 16_777_216.0
    ///
    /// // Good
    /// let a: f64 = 0.123_456_789_9;
    /// let b: f64 = 16_777_216.0;
    /// ```
    pub EXCESSIVE_PRECISION,
    correctness,
    "excessive precision for float literal"
}

declare_lint_pass!(ExcessivePrecision => [EXCESSIVE_PRECISION]);

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for ExcessivePrecision {
    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx hir::Expr<'_>) {
        if_chain! {
            let ty = cx.tables.expr_ty(expr);
            if let ty::Float(fty) = ty.kind;
            if let hir::ExprKind::Lit(ref lit) = expr.kind;
            if let LitKind::Float(sym, lit_float_ty) = lit.node;
            then {
                let sym_str = sym.as_str();
                let formatter = FloatFormat::new(&sym_str);
                // Try to bail out if the float is for sure fine.
                // If its within the 2 decimal digits of being out of precision we
                // check if the parsed representation is the same as the string
                // since we'll need the truncated string anyway.
                let digits = count_digits(&sym_str);
                let max = max_digits(fty);
                let float_str = match fty {
                    FloatTy::F32 => sym_str.parse::<f32>().map(|f| formatter.format(f)),
                    FloatTy::F64 => sym_str.parse::<f64>().map(|f| formatter.format(f)),
                }.unwrap();
                let type_suffix = match lit_float_ty {
                    LitFloatType::Suffixed(FloatTy::F32) => Some("f32"),
                    LitFloatType::Suffixed(FloatTy::F64) => Some("f64"),
                    _ => None
                };

                if is_whole_number(&sym_str, fty) {
                    // Normalize the literal by stripping the fractional portion
                    if sym_str.split('.').next().unwrap() != float_str {
                        span_lint_and_sugg(
                            cx,
                            EXCESSIVE_PRECISION,
                            expr.span,
                            "literal cannot be represented as the underlying type without loss of precision",
                            "consider changing the type or replacing it with",
                            format_numeric_literal(format!("{}.0", float_str).as_str(), type_suffix, true),
                            Applicability::MachineApplicable,
                        );
                    }
                } else if digits > max as usize && sym_str != float_str {
                    span_lint_and_sugg(
                        cx,
                        EXCESSIVE_PRECISION,
                        expr.span,
                        "float has excessive precision",
                        "consider changing the type or truncating it to",
                        format_numeric_literal(&float_str, type_suffix, true),
                        Applicability::MachineApplicable,
                    );
                }
            }
        }
    }
}

// Checks whether a float literal is a whole number
#[must_use]
fn is_whole_number(sym_str: &str, fty: FloatTy) -> bool {
    match fty {
        FloatTy::F32 => sym_str.parse::<f32>().unwrap().fract() == 0.0,
        FloatTy::F64 => sym_str.parse::<f64>().unwrap().fract() == 0.0,
    }
}

#[must_use]
fn max_digits(fty: FloatTy) -> u32 {
    match fty {
        FloatTy::F32 => f32::DIGITS,
        FloatTy::F64 => f64::DIGITS,
    }
}

/// Counts the digits excluding leading zeros
#[must_use]
fn count_digits(s: &str) -> usize {
    // Note that s does not contain the f32/64 suffix, and underscores have been stripped
    s.chars()
        .filter(|c| *c != '-' && *c != '.')
        .take_while(|c| *c != 'e' && *c != 'E')
        .fold(0, |count, c| {
            // leading zeros
            if c == '0' && count == 0 {
                count
            } else {
                count + 1
            }
        })
}

enum FloatFormat {
    LowerExp,
    UpperExp,
    Normal,
}
impl FloatFormat {
    #[must_use]
    fn new(s: &str) -> Self {
        s.chars()
            .find_map(|x| match x {
                'e' => Some(Self::LowerExp),
                'E' => Some(Self::UpperExp),
                _ => None,
            })
            .unwrap_or(Self::Normal)
    }
    fn format<T>(&self, f: T) -> String
    where
        T: fmt::UpperExp + fmt::LowerExp + fmt::Display,
    {
        match self {
            Self::LowerExp => format!("{:e}", f),
            Self::UpperExp => format!("{:E}", f),
            Self::Normal => format!("{}", f),
        }
    }
}
