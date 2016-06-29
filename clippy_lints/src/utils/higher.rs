//! This module contains functions for retrieve the original AST from lowered `hir`.

use rustc::hir;
use syntax::ast;
use utils::{match_path, paths};

/// Convert a hir binary operator to the corresponding `ast` type.
pub fn binop(op: hir::BinOp_) -> ast::BinOpKind {
    match op {
        hir::BiEq => ast::BinOpKind::Eq,
        hir::BiGe => ast::BinOpKind::Ge,
        hir::BiGt => ast::BinOpKind::Gt,
        hir::BiLe => ast::BinOpKind::Le,
        hir::BiLt => ast::BinOpKind::Lt,
        hir::BiNe => ast::BinOpKind::Ne,
        hir::BiOr => ast::BinOpKind::Or,
        hir::BiAdd => ast::BinOpKind::Add,
        hir::BiAnd => ast::BinOpKind::And,
        hir::BiBitAnd => ast::BinOpKind::BitAnd,
        hir::BiBitOr => ast::BinOpKind::BitOr,
        hir::BiBitXor => ast::BinOpKind::BitXor,
        hir::BiDiv => ast::BinOpKind::Div,
        hir::BiMul => ast::BinOpKind::Mul,
        hir::BiRem => ast::BinOpKind::Rem,
        hir::BiShl => ast::BinOpKind::Shl,
        hir::BiShr => ast::BinOpKind::Shr,
        hir::BiSub => ast::BinOpKind::Sub,
    }
}

/// Represent a range akin to `ast::ExprKind::Range`.
#[derive(Debug, Copy, Clone)]
pub struct Range<'a> {
    pub start: Option<&'a hir::Expr>,
    pub end: Option<&'a hir::Expr>,
    pub limits: ast::RangeLimits,
}

/// Higher a `hir` range to something similar to `ast::ExprKind::Range`.
pub fn range(expr: &hir::Expr) -> Option<Range> {
    // To be removed when ranges get stable.
    fn unwrap_unstable(expr: &hir::Expr) -> &hir::Expr {
        if let hir::ExprBlock(ref block) = expr.node {
            if block.rules == hir::BlockCheckMode::PushUnstableBlock || block.rules == hir::BlockCheckMode::PopUnstableBlock {
                if let Some(ref expr) = block.expr {
                    return expr;
                }
            }
        }

        expr
    }

    fn get_field<'a>(name: &str, fields: &'a [hir::Field]) -> Option<&'a hir::Expr> {
        let expr = &fields.iter()
                          .find(|field| field.name.node.as_str() == name)
                          .unwrap_or_else(|| panic!("missing {} field for range", name))
                          .expr;

        Some(unwrap_unstable(expr))
    }

    // The range syntax is expanded to literal paths starting with `core` or `std` depending on
    // `#[no_std]`. Testing both instead of resolving the paths.

    match unwrap_unstable(expr).node {
        hir::ExprPath(None, ref path) => {
            if match_path(path, &paths::RANGE_FULL_STD) || match_path(path, &paths::RANGE_FULL) {
                Some(Range {
                    start: None,
                    end: None,
                    limits: ast::RangeLimits::HalfOpen,
                })
            } else {
                None
            }
        }
        hir::ExprStruct(ref path, ref fields, None) => {
            if match_path(path, &paths::RANGE_FROM_STD) || match_path(path, &paths::RANGE_FROM) {
                Some(Range {
                    start: get_field("start", fields),
                    end: None,
                    limits: ast::RangeLimits::HalfOpen,
                })
            } else if match_path(path, &paths::RANGE_INCLUSIVE_NON_EMPTY_STD) ||
               match_path(path, &paths::RANGE_INCLUSIVE_NON_EMPTY) {
                Some(Range {
                    start: get_field("start", fields),
                    end: get_field("end", fields),
                    limits: ast::RangeLimits::Closed,
                })
            } else if match_path(path, &paths::RANGE_STD) || match_path(path, &paths::RANGE) {
                Some(Range {
                    start: get_field("start", fields),
                    end: get_field("end", fields),
                    limits: ast::RangeLimits::HalfOpen,
                })
            } else if match_path(path, &paths::RANGE_TO_INCLUSIVE_STD) || match_path(path, &paths::RANGE_TO_INCLUSIVE) {
                Some(Range {
                    start: None,
                    end: get_field("end", fields),
                    limits: ast::RangeLimits::Closed,
                })
            } else if match_path(path, &paths::RANGE_TO_STD) || match_path(path, &paths::RANGE_TO) {
                Some(Range {
                    start: None,
                    end: get_field("end", fields),
                    limits: ast::RangeLimits::HalfOpen,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

