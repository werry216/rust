//! lint on manually implemented checked conversions that could be transformed into try_from

use if_chain::if_chain;
use rustc::hir::*;
use rustc::lint::{in_external_macro, LateContext, LateLintPass, LintArray, LintContext, LintPass};
use rustc::{declare_lint_pass, declare_tool_lint};
use syntax::ast::LitKind;

use crate::utils::{span_lint, SpanlessEq};

declare_clippy_lint! {
    /// **What it does:** Checks for explicit bounds checking when casting.
    ///
    /// **Why is this bad?** Reduces the readability of statements & is error prone.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// # let foo: u32 = 5;
    /// # let _ =
    /// foo <= i32::max_value() as u32
    /// # ;
    /// ```
    ///
    /// Could be written:
    ///
    /// ```rust
    /// # let _ =
    /// i32::try_from(foo).is_ok()
    /// # ;
    /// ```
    pub CHECKED_CONVERSIONS,
    pedantic,
    "`try_from` could replace manual bounds checking when casting"
}

declare_lint_pass!(CheckedConversions => [CHECKED_CONVERSIONS]);

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for CheckedConversions {
    fn check_expr(&mut self, cx: &LateContext<'_, '_>, item: &Expr) {
        let result = if_chain! {
            if !in_external_macro(cx.sess(), item.span);
            if let ExprKind::Binary(op, ref left, ref right) = &item.node;

            then {
                match op.node {
                    BinOpKind::Ge | BinOpKind::Le => single_check(item),
                    BinOpKind::And => double_check(cx, left, right),
                    _ => None,
                }
            } else {
                None
            }
        };

        if let Some(cv) = result {
            span_lint(
                cx,
                CHECKED_CONVERSIONS,
                item.span,
                &format!(
                    "Checked cast can be simplified: `{}::try_from`",
                    cv.to_type.unwrap_or("IntegerType".to_string()),
                ),
            );
        }
    }
}

/// Searches for a single check from unsigned to _ is done
/// todo: check for case signed -> larger unsigned == only x >= 0
fn single_check(expr: &Expr) -> Option<Conversion<'_>> {
    check_upper_bound(expr).filter(|cv| cv.cvt == ConversionType::FromUnsigned)
}

/// Searches for a combination of upper & lower bound checks
fn double_check<'a>(cx: &LateContext<'_, '_>, left: &'a Expr, right: &'a Expr) -> Option<Conversion<'a>> {
    let upper_lower = |l, r| {
        let upper = check_upper_bound(l);
        let lower = check_lower_bound(r);

        transpose(upper, lower).and_then(|(l, r)| l.combine(r, cx))
    };

    upper_lower(left, right).or_else(|| upper_lower(right, left))
}

/// Contains the result of a tried conversion check
#[derive(Clone, Debug)]
struct Conversion<'a> {
    cvt: ConversionType,
    expr_to_cast: &'a Expr,
    to_type: Option<String>,
}

/// The kind of conversion that is checked
#[derive(Copy, Clone, Debug, PartialEq)]
enum ConversionType {
    SignedToUnsigned,
    SignedToSigned,
    FromUnsigned,
}

impl<'a> Conversion<'a> {
    /// Combine multiple conversions if the are compatible
    pub fn combine(self, other: Self, cx: &LateContext<'_, '_>) -> Option<Conversion<'a>> {
        if self.is_compatible(&other, cx) {
            // Prefer a Conversion that contains a type-constraint
            Some(if self.to_type.is_some() { self } else { other })
        } else {
            None
        }
    }

    /// Checks if two conversions are compatible
    /// same type of conversion, same 'castee' and same 'to type'
    pub fn is_compatible(&self, other: &Self, cx: &LateContext<'_, '_>) -> bool {
        (self.cvt == other.cvt)
            && (SpanlessEq::new(cx).eq_expr(self.expr_to_cast, other.expr_to_cast))
            && (self.has_compatible_to_type(other))
    }

    /// Checks if the to-type is the same (if there is a type constraint)
    fn has_compatible_to_type(&self, other: &Self) -> bool {
        transpose(self.to_type.as_ref(), other.to_type.as_ref())
            .map(|(l, r)| l == r)
            .unwrap_or(true)
    }

    /// Try to construct a new conversion if the conversion type is valid
    fn try_new<'b>(expr_to_cast: &'a Expr, from_type: &'b str, to_type: String) -> Option<Conversion<'a>> {
        ConversionType::try_new(from_type, &to_type).map(|cvt| Conversion {
            cvt,
            expr_to_cast,
            to_type: Some(to_type),
        })
    }

    /// Construct a new conversion without type constraint
    fn new_any(expr_to_cast: &'a Expr) -> Conversion<'a> {
        Conversion {
            cvt: ConversionType::SignedToUnsigned,
            expr_to_cast,
            to_type: None,
        }
    }
}

impl ConversionType {
    /// Creates a conversion type if the type is allowed & conversion is valid
    fn try_new(from: &str, to: &str) -> Option<ConversionType> {
        if UNSIGNED_TYPES.contains(&from) {
            Some(ConversionType::FromUnsigned)
        } else if SIGNED_TYPES.contains(&from) {
            if UNSIGNED_TYPES.contains(&to) {
                Some(ConversionType::SignedToUnsigned)
            } else if SIGNED_TYPES.contains(&to) {
                Some(ConversionType::SignedToSigned)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Check for `expr <= (to_type::max_value() as from_type)`
fn check_upper_bound(expr: &Expr) -> Option<Conversion<'_>> {
    if_chain! {
         if let ExprKind::Binary(ref op, ref left, ref right) = &expr.node;
         if let Some((candidate, check)) = normalize_le_ge(op, left, right);
         if let Some((from, to)) = get_types_from_cast(check, "max_value", INT_TYPES);

         then {
             Conversion::try_new(candidate, &from, to)
         } else {
            None
        }
    }
}

/// Check for `expr >= 0|(to_type::min_value() as from_type)`
fn check_lower_bound(expr: &Expr) -> Option<Conversion<'_>> {
    fn check_function<'a>(candidate: &'a Expr, check: &'a Expr) -> Option<Conversion<'a>> {
        (check_lower_bound_zero(candidate, check)).or_else(|| (check_lower_bound_min(candidate, check)))
    }

    // First of we need a binary containing the expression & the cast
    if let ExprKind::Binary(ref op, ref left, ref right) = &expr.node {
        normalize_le_ge(op, right, left).and_then(|(l, r)| check_function(l, r))
    } else {
        None
    }
}

/// Check for `expr >= 0`
fn check_lower_bound_zero<'a>(candidate: &'a Expr, check: &'a Expr) -> Option<Conversion<'a>> {
    if_chain! {
        if let ExprKind::Lit(ref lit) = &check.node;
        if let LitKind::Int(0, _) = &lit.node;

        then {
            Some(Conversion::new_any(candidate))
        } else {
            None
        }
    }
}

/// Check for `expr >= (to_type::min_value() as from_type)`
fn check_lower_bound_min<'a>(candidate: &'a Expr, check: &'a Expr) -> Option<Conversion<'a>> {
    if let Some((from, to)) = get_types_from_cast(check, "min_value", SIGNED_TYPES) {
        Conversion::try_new(candidate, &from, to)
    } else {
        None
    }
}

/// Tries to extract the from- and to-type from a cast expression
fn get_types_from_cast(expr: &Expr, func: &str, types: &[&str]) -> Option<(String, String)> {
    // `to_type::maxmin_value() as from_type`
    let call_from_cast: Option<(&Expr, String)> = if_chain! {
        // to_type::maxmin_value(), from_type
        if let ExprKind::Cast(ref limit, ref from_type) = &expr.node;
        if let TyKind::Path(ref from_type_path) = &from_type.node;
        if let Some(from_type_str) = int_ty_to_str(from_type_path);

        then {
            Some((limit, from_type_str.to_string()))
        } else {
            None
        }
    };

    // `from_type::from(to_type::maxmin_value())`
    let limit_from: Option<(&Expr, String)> = call_from_cast.or_else(|| {
        if_chain! {
            // `from_type::from, to_type::maxmin_value()`
            if let ExprKind::Call(ref from_func, ref args) = &expr.node;
            // `to_type::maxmin_value()`
            if args.len() == 1;
            if let limit = &args[0];
            // `from_type::from`
            if let ExprKind::Path(ref path) = &from_func.node;
            if let Some(from_type) = get_implementing_type(path, INT_TYPES, "from");

            then {
                Some((limit, from_type))
            } else {
                None
            }
        }
    });

    if let Some((limit, from_type)) = limit_from {
        if_chain! {
            if let ExprKind::Call(ref fun_name, _) = &limit.node;
            // `to_type, maxmin_value`
            if let ExprKind::Path(ref path) = &fun_name.node;
            // `to_type`
            if let Some(to_type) = get_implementing_type(path, types, func);

            then {
                Some((from_type, to_type))
            } else {
                None
            }
        }
    } else {
        None
    }
}

/// Gets the type which implements the called function
fn get_implementing_type(path: &QPath, candidates: &[&str], function: &str) -> Option<String> {
    if_chain! {
        if let QPath::TypeRelative(ref ty, ref path) = &path;
        if path.ident.name == function;
        if let TyKind::Path(QPath::Resolved(None, ref tp)) = &ty.node;
        if let [int] = &*tp.segments;
        let name = int.ident.as_str().get();
        if candidates.contains(&name);

        then {
            Some(name.to_string())
        } else {
            None
        }
    }
}

/// Gets the type as a string, if it is a supported integer
fn int_ty_to_str(path: &QPath) -> Option<&str> {
    if_chain! {
        if let QPath::Resolved(_, ref path) = *path;
        if let [ty] = &*path.segments;

        then {
            INT_TYPES
                .into_iter()
                .find(|c| (&ty.ident.name) == *c)
                .cloned()
        } else {
            None
        }
    }
}

/// (Option<T>, Option<U>) -> Option<(T, U)>
fn transpose<T, U>(lhs: Option<T>, rhs: Option<U>) -> Option<(T, U)> {
    match (lhs, rhs) {
        (Some(l), Some(r)) => Some((l, r)),
        _ => None,
    }
}

/// Will return the expressions as if they were expr1 <= expr2
fn normalize_le_ge<'a>(op: &'a BinOp, left: &'a Expr, right: &'a Expr) -> Option<(&'a Expr, &'a Expr)> {
    match op.node {
        BinOpKind::Le => Some((left, right)),
        BinOpKind::Ge => Some((right, left)),
        _ => None,
    }
}

const UNSIGNED_TYPES: &[&str] = &["u8", "u16", "u32", "u64", "u128", "usize"];
const SIGNED_TYPES: &[&str] = &["i8", "i16", "i32", "i64", "i128", "isize"];
const INT_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
];
