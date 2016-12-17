use reexport::*;
use rustc::hir::*;
use rustc::hir::intravisit::FnKind;
use rustc::lint::*;
use rustc::middle::const_val::ConstVal;
use rustc::ty;
use rustc_const_eval::EvalHint::ExprTypeChecked;
use rustc_const_eval::eval_const_expr_partial;
use rustc_const_math::ConstFloat;
use syntax::codemap::{Span, Spanned, ExpnFormat};
use utils::{
    get_item_name, get_parent_expr, implements_trait, in_macro, is_integer_literal, match_path,
    snippet, span_lint, span_lint_and_then, walk_ptrs_ty, last_path_segment
};
use utils::sugg::Sugg;

/// **What it does:** Checks for function arguments and let bindings denoted as `ref`.
///
/// **Why is this bad?** The `ref` declaration makes the function take an owned
/// value, but turns the argument into a reference (which means that the value
/// is destroyed when exiting the function). This adds not much value: either
/// take a reference type, or take an owned value and create references in the
/// body.
///
/// For let bindings, `let x = &foo;` is preferred over `let ref x = foo`. The
/// type of `x` is more obvious with the former.
///
/// **Known problems:** If the argument is dereferenced within the function,
/// removing the `ref` will lead to errors. This can be fixed by removing the
/// dereferences, e.g. changing `*x` to `x` within the function.
///
/// **Example:**
/// ```rust
/// fn foo(ref x: u8) -> bool { .. }
/// ```
declare_lint! {
    pub TOPLEVEL_REF_ARG,
    Warn,
    "an entire binding declared as `ref`, in a function argument or a `let` statement"
}

/// **What it does:** Checks for comparisons to NaN.
///
/// **Why is this bad?** NaN does not compare meaningfully to anything – not
/// even itself – so those comparisons are simply wrong.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// x == NAN
/// ```
declare_lint! {
    pub CMP_NAN,
    Deny,
    "comparisons to NAN, which will always return false, probably not intended"
}

/// **What it does:** Checks for (in-)equality comparisons on floating-point
/// values (apart from zero), except in functions called `*eq*` (which probably
/// implement equality for a type involving floats).
///
/// **Why is this bad?** Floating point calculations are usually imprecise, so
/// asking if two values are *exactly* equal is asking for trouble. For a good
/// guide on what to do, see [the floating point
/// guide](http://www.floating-point-gui.de/errors/comparison).
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// y == 1.23f64
/// y != x  // where both are floats
/// ```
declare_lint! {
    pub FLOAT_CMP,
    Warn,
    "using `==` or `!=` on float values instead of comparing difference with an epsilon"
}

/// **What it does:** Checks for conversions to owned values just for the sake
/// of a comparison.
///
/// **Why is this bad?** The comparison can operate on a reference, so creating
/// an owned value effectively throws it away directly afterwards, which is
/// needlessly consuming code and heap space.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// x.to_owned() == y
/// ```
declare_lint! {
    pub CMP_OWNED,
    Warn,
    "creating owned instances for comparing with others, e.g. `x == \"foo\".to_string()`"
}

/// **What it does:** Checks for getting the remainder of a division by one.
///
/// **Why is this bad?** The result can only ever be zero. No one will write
/// such code deliberately, unless trying to win an Underhanded Rust
/// Contest. Even for that contest, it's probably a bad idea. Use something more
/// underhanded.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// x % 1
/// ```
declare_lint! {
    pub MODULO_ONE,
    Warn,
    "taking a number modulo 1, which always returns 0"
}

/// **What it does:** Checks for patterns in the form `name @ _`.
///
/// **Why is this bad?** It's almost always more readable to just use direct bindings.
///
/// **Known problems:** None.
///
/// **Example:**
/// ```rust
/// match v {
///     Some(x) => (),
///     y @ _   => (), // easier written as `y`,
/// }
/// ```
declare_lint! {
    pub REDUNDANT_PATTERN,
    Warn,
    "using `name @ _` in a pattern"
}

/// **What it does:** Checks for the use of bindings with a single leading underscore.
///
/// **Why is this bad?** A single leading underscore is usually used to indicate
/// that a binding will not be used. Using such a binding breaks this
/// expectation.
///
/// **Known problems:** The lint does not work properly with desugaring and
/// macro, it has been allowed in the mean time.
///
/// **Example:**
/// ```rust
/// let _x = 0;
/// let y = _x + 1; // Here we are using `_x`, even though it has a leading underscore.
///                 // We should rename `_x` to `x`
/// ```
declare_lint! {
    pub USED_UNDERSCORE_BINDING,
    Allow,
    "using a binding which is prefixed with an underscore"
}

#[derive(Copy, Clone)]
pub struct Pass;

impl LintPass for Pass {
    fn get_lints(&self) -> LintArray {
        lint_array!(TOPLEVEL_REF_ARG, CMP_NAN, FLOAT_CMP, CMP_OWNED, MODULO_ONE, REDUNDANT_PATTERN,
                    USED_UNDERSCORE_BINDING)
    }
}

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for Pass {
    fn check_fn(&mut self, cx: &LateContext<'a, 'tcx>, k: FnKind<'tcx>, decl: &'tcx FnDecl, _: &'tcx Expr, _: Span, _: NodeId) {
        if let FnKind::Closure(_) = k {
            // Does not apply to closures
            return;
        }
        for arg in &decl.inputs {
            if let PatKind::Binding(BindByRef(_), _, _, _) = arg.pat.node {
                span_lint(cx,
                          TOPLEVEL_REF_ARG,
                          arg.pat.span,
                          "`ref` directly on a function argument is ignored. Consider using a reference type instead.");
            }
        }
    }

    fn check_stmt(&mut self, cx: &LateContext<'a, 'tcx>, s: &'tcx Stmt) {
        if_let_chain! {[
            let StmtDecl(ref d, _) = s.node,
            let DeclLocal(ref l) = d.node,
            let PatKind::Binding(BindByRef(mt), _, i, None) = l.pat.node,
            let Some(ref init) = l.init
        ], {
            let init = Sugg::hir(cx, init, "..");
            let (mutopt,initref) = if mt == Mutability::MutMutable {
                ("mut ", init.mut_addr())
            } else {
                ("", init.addr())
            };
            let tyopt = if let Some(ref ty) = l.ty {
                format!(": &{mutopt}{ty}", mutopt=mutopt, ty=snippet(cx, ty.span, "_"))
            } else {
                "".to_owned()
            };
            span_lint_and_then(cx,
                TOPLEVEL_REF_ARG,
                l.pat.span,
                "`ref` on an entire `let` pattern is discouraged, take a reference with `&` instead",
                |db| {
                    db.span_suggestion(s.span,
                                       "try",
                                       format!("let {name}{tyopt} = {initref};",
                                               name=snippet(cx, i.span, "_"),
                                               tyopt=tyopt,
                                               initref=initref));
                }
            );
        }}
    }

    fn check_expr(&mut self, cx: &LateContext<'a, 'tcx>, expr: &'tcx Expr) {
        if let ExprBinary(ref cmp, ref left, ref right) = expr.node {
            let op = cmp.node;
            if op.is_comparison() {
                if let ExprPath(QPath::Resolved(_, ref path)) = left.node {
                    check_nan(cx, path, expr.span);
                }
                if let ExprPath(QPath::Resolved(_, ref path)) = right.node {
                    check_nan(cx, path, expr.span);
                }
                check_to_owned(cx, left, right, true, cmp.span);
                check_to_owned(cx, right, left, false, cmp.span)
            }
            if (op == BiEq || op == BiNe) && (is_float(cx, left) || is_float(cx, right)) {
                if is_allowed(cx, left) || is_allowed(cx, right) {
                    return;
                }
                if let Some(name) = get_item_name(cx, expr) {
                    let name = &*name.as_str();
                    if name == "eq" || name == "ne" || name == "is_nan" || name.starts_with("eq_") ||
                        name.ends_with("_eq") {
                        return;
                    }
                }
                span_lint_and_then(cx,
                                   FLOAT_CMP,
                                   expr.span,
                                   "strict comparison of f32 or f64",
                                   |db| {
                                       let lhs = Sugg::hir(cx, left, "..");
                                       let rhs = Sugg::hir(cx, right, "..");

                                       db.span_suggestion(expr.span,
                                                          "consider comparing them within some error",
                                                          format!("({}).abs() < error", lhs - rhs));
                                       db.span_note(expr.span, "std::f32::EPSILON and std::f64::EPSILON are available.");
                                   });
            } else if op == BiRem && is_integer_literal(right, 1) {
                span_lint(cx, MODULO_ONE, expr.span, "any number modulo 1 will be 0");
            }
        }
        if in_attributes_expansion(cx, expr) {
            // Don't lint things expanded by #[derive(...)], etc
            return;
        }
        let binding = match expr.node {
            ExprPath(ref qpath) => {
                let binding = last_path_segment(qpath).name.as_str();
                if binding.starts_with('_') &&
                    !binding.starts_with("__") &&
                    &*binding != "_result" && // FIXME: #944
                    is_used(cx, expr) &&
                    // don't lint if the declaration is in a macro
                    non_macro_local(cx, &cx.tcx.tables().qpath_def(qpath, expr.id)) {
                    Some(binding)
                } else {
                    None
                }
            }
            ExprField(_, spanned) => {
                let name = spanned.node.as_str();
                if name.starts_with('_') && !name.starts_with("__") {
                    Some(name)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(binding) = binding {
            span_lint(cx,
                      USED_UNDERSCORE_BINDING,
                      expr.span,
                      &format!("used binding `{}` which is prefixed with an underscore. A leading \
                                underscore signals that a binding will not be used.", binding));
        }
    }

    fn check_pat(&mut self, cx: &LateContext<'a, 'tcx>, pat: &'tcx Pat) {
        if let PatKind::Binding(_, _, ref ident, Some(ref right)) = pat.node {
            if right.node == PatKind::Wild {
                span_lint(cx,
                          REDUNDANT_PATTERN,
                          pat.span,
                          &format!("the `{} @ _` pattern can be written as just `{}`",
                                   ident.node,
                                   ident.node));
            }
        }
    }
}

fn check_nan(cx: &LateContext, path: &Path, span: Span) {
    path.segments.last().map(|seg| {
        if &*seg.name.as_str() == "NAN" {
            span_lint(cx,
                      CMP_NAN,
                      span,
                      "doomed comparison with NAN, use `std::{f32,f64}::is_nan()` instead");
        }
    });
}

fn is_allowed(cx: &LateContext, expr: &Expr) -> bool {
    let res = eval_const_expr_partial(cx.tcx, expr, ExprTypeChecked, None);
    if let Ok(ConstVal::Float(val)) = res {
        use std::cmp::Ordering;

        let zero = ConstFloat::FInfer {
            f32: 0.0,
            f64: 0.0,
        };

        let infinity = ConstFloat::FInfer {
            f32: ::std::f32::INFINITY,
            f64: ::std::f64::INFINITY,
        };

        let neg_infinity = ConstFloat::FInfer {
            f32: ::std::f32::NEG_INFINITY,
            f64: ::std::f64::NEG_INFINITY,
        };

        val.try_cmp(zero) == Ok(Ordering::Equal)
            || val.try_cmp(infinity) == Ok(Ordering::Equal)
            || val.try_cmp(neg_infinity) == Ok(Ordering::Equal)
    } else {
        false
    }
}

fn is_float(cx: &LateContext, expr: &Expr) -> bool {
    matches!(walk_ptrs_ty(cx.tcx.tables().expr_ty(expr)).sty, ty::TyFloat(_))
}

fn check_to_owned(cx: &LateContext, expr: &Expr, other: &Expr, left: bool, op: Span) {
    let (arg_ty, snip) = match expr.node {
        ExprMethodCall(Spanned { node: ref name, .. }, _, ref args) if args.len() == 1 => {
            let name = &*name.as_str();
            if name == "to_string" || name == "to_owned" && is_str_arg(cx, args) {
                (cx.tcx.tables().expr_ty(&args[0]), snippet(cx, args[0].span, ".."))
            } else {
                return;
            }
        }
        ExprCall(ref path, ref v) if v.len() == 1 => {
            if let ExprPath(ref path) = path.node {
                if match_path(path, &["String", "from_str"]) || match_path(path, &["String", "from"]) {
                    (cx.tcx.tables().expr_ty(&v[0]), snippet(cx, v[0].span, ".."))
                } else {
                    return;
                }
            } else {
                return;
            }
        }
        _ => return,
    };

    let other_ty = cx.tcx.tables().expr_ty(other);
    let partial_eq_trait_id = match cx.tcx.lang_items.eq_trait() {
        Some(id) => id,
        None => return,
    };

    if !implements_trait(cx, arg_ty, partial_eq_trait_id, vec![other_ty]) {
        return;
    }

    if left {
        span_lint(cx,
                  CMP_OWNED,
                  expr.span,
                  &format!("this creates an owned instance just for comparison. Consider using `{} {} {}` to \
                            compare without allocation",
                           snip,
                           snippet(cx, op, "=="),
                           snippet(cx, other.span, "..")));
    } else {
        span_lint(cx,
                  CMP_OWNED,
                  expr.span,
                  &format!("this creates an owned instance just for comparison. Consider using `{} {} {}` to \
                            compare without allocation",
                           snippet(cx, other.span, ".."),
                           snippet(cx, op, "=="),
                           snip));
    }

}

fn is_str_arg(cx: &LateContext, args: &[Expr]) -> bool {
    args.len() == 1 &&
        matches!(walk_ptrs_ty(cx.tcx.tables().expr_ty(&args[0])).sty, ty::TyStr)
}

/// Heuristic to see if an expression is used. Should be compatible with `unused_variables`'s idea
/// of what it means for an expression to be "used".
fn is_used(cx: &LateContext, expr: &Expr) -> bool {
    if let Some(parent) = get_parent_expr(cx, expr) {
        match parent.node {
            ExprAssign(_, ref rhs) |
            ExprAssignOp(_, _, ref rhs) => **rhs == *expr,
            _ => is_used(cx, parent),
        }
    } else {
        true
    }
}

/// Test whether an expression is in a macro expansion (e.g. something generated by
/// `#[derive(...)`] or the like).
fn in_attributes_expansion(cx: &LateContext, expr: &Expr) -> bool {
    cx.sess().codemap().with_expn_info(expr.span.expn_id, |info_opt| {
        info_opt.map_or(false, |info| {
            matches!(info.callee.format, ExpnFormat::MacroAttribute(_))
        })
    })
}

/// Test whether `def` is a variable defined outside a macro.
fn non_macro_local(cx: &LateContext, def: &def::Def) -> bool {
    match *def {
        def::Def::Local(id) | def::Def::Upvar(id, _, _) => {
            if let Some(span) = cx.tcx.map.span_if_local(id) {
                !in_macro(cx, span)
            } else {
                true
            }
        }
        _ => false,
    }
}
