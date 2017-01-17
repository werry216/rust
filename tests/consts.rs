#![feature(rustc_private)]

extern crate clippy_lints;
extern crate rustc;
extern crate rustc_const_eval;
extern crate rustc_const_math;
extern crate syntax;

use clippy_lints::consts::{constant_simple, Constant, FloatWidth};
use rustc_const_math::ConstInt;
use rustc::hir::*;
use syntax::ast::{LitIntType, LitKind, NodeId, StrStyle};
use syntax::codemap::{Spanned, COMMAND_LINE_SP};
use syntax::symbol::Symbol;
use syntax::ptr::P;
use syntax::util::ThinVec;

fn spanned<T>(t: T) -> Spanned<T> {
    Spanned {
        node: t,
        span: COMMAND_LINE_SP,
    }
}

fn expr(n: Expr_) -> Expr {
    Expr {
        id: NodeId::new(1),
        node: n,
        span: COMMAND_LINE_SP,
        attrs: ThinVec::new(),
    }
}

fn lit(l: LitKind) -> Expr {
    expr(ExprLit(P(spanned(l))))
}

fn binop(op: BinOp_, l: Expr, r: Expr) -> Expr {
    expr(ExprBinary(spanned(op), P(l), P(r)))
}

fn check(expect: Constant, expr: &Expr) {
    assert_eq!(Some(expect), constant_simple(expr))
}

const TRUE: Constant = Constant::Bool(true);
const FALSE: Constant = Constant::Bool(false);
const ZERO: Constant = Constant::Int(ConstInt::Infer(0));
const ONE: Constant = Constant::Int(ConstInt::Infer(1));
const TWO: Constant = Constant::Int(ConstInt::Infer(2));

#[test]
fn test_lit() {
    check(TRUE, &lit(LitKind::Bool(true)));
    check(FALSE, &lit(LitKind::Bool(false)));
    check(ZERO, &lit(LitKind::Int(0, LitIntType::Unsuffixed)));
    check(Constant::Str("cool!".into(), StrStyle::Cooked),
          &lit(LitKind::Str(Symbol::intern("cool!"), StrStyle::Cooked)));
}

#[test]
fn test_ops() {
    check(TRUE, &binop(BiOr, lit(LitKind::Bool(false)), lit(LitKind::Bool(true))));
    check(FALSE, &binop(BiAnd, lit(LitKind::Bool(false)), lit(LitKind::Bool(true))));

    let litzero = lit(LitKind::Int(0, LitIntType::Unsuffixed));
    let litone = lit(LitKind::Int(1, LitIntType::Unsuffixed));
    check(TRUE, &binop(BiEq, litzero.clone(), litzero.clone()));
    check(TRUE, &binop(BiGe, litzero.clone(), litzero.clone()));
    check(TRUE, &binop(BiLe, litzero.clone(), litzero.clone()));
    check(FALSE, &binop(BiNe, litzero.clone(), litzero.clone()));
    check(FALSE, &binop(BiGt, litzero.clone(), litzero.clone()));
    check(FALSE, &binop(BiLt, litzero.clone(), litzero.clone()));

    check(ZERO, &binop(BiAdd, litzero.clone(), litzero.clone()));
    check(TWO, &binop(BiAdd, litone.clone(), litone.clone()));
    check(ONE, &binop(BiSub, litone.clone(), litzero.clone()));
    check(ONE, &binop(BiMul, litone.clone(), litone.clone()));
    check(ONE, &binop(BiDiv, litone.clone(), litone.clone()));

    let half_any = Constant::Float("0.5".into(), FloatWidth::Any);
    let half32 = Constant::Float("0.5".into(), FloatWidth::F32);
    let half64 = Constant::Float("0.5".into(), FloatWidth::F64);
    let pos_zero = Constant::Float("0.0".into(), FloatWidth::F64);
    let neg_zero = Constant::Float("-0.0".into(), FloatWidth::F64);

    assert_eq!(pos_zero, pos_zero);
    assert_eq!(neg_zero, neg_zero);
    assert_eq!(None, pos_zero.partial_cmp(&neg_zero));

    assert_eq!(half_any, half32);
    assert_eq!(half_any, half64);
    assert_eq!(half32, half64); // for transitivity

    assert_eq!(Constant::Int(ConstInt::Infer(0)), Constant::Int(ConstInt::U8(0)));
    assert_eq!(Constant::Int(ConstInt::Infer(0)), Constant::Int(ConstInt::I8(0)));
    assert_eq!(Constant::Int(ConstInt::InferSigned(-1)), Constant::Int(ConstInt::I8(-1)));
}
