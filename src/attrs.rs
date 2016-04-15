//! checks for attributes

use reexport::*;
use rustc::lint::*;
use rustc::hir::*;
use semver::Version;
use syntax::ast::{Attribute, Lit, LitKind, MetaItemKind};
use syntax::attr::*;
use syntax::codemap::Span;
use utils::{in_macro, match_path, span_lint};
use utils::paths;

/// **What it does:** This lint checks for items annotated with `#[inline(always)]`, unless the annotated function is empty or simply panics.
///
/// **Why is this bad?** While there are valid uses of this annotation (and once you know when to use it, by all means `allow` this lint), it's a common newbie-mistake to pepper one's code with it.
///
/// As a rule of thumb, before slapping `#[inline(always)]` on a function, measure if that additional function call really affects your runtime profile sufficiently to make up for the increase in compile time.
///
/// **Known problems:** False positives, big time. This lint is meant to be deactivated by everyone doing serious performance work. This means having done the measurement.
///
/// **Example:**
/// ```
/// #[inline(always)]
/// fn not_quite_hot_code(..) { ... }
/// ```
declare_lint! {
    pub INLINE_ALWAYS, Warn,
    "`#[inline(always)]` is a bad idea in most cases"
}

/// **What it does:** This lint checks for `#[deprecated]` annotations with a `since` field that is not a valid semantic version..
///
/// **Why is this bad?** For checking the version of the deprecation, it must be valid semver. Failing that, the contained information is useless.
///
/// **Known problems:** None
///
/// **Example:**
/// ```
/// #[deprecated(since = "forever")]
/// fn something_else(..) { ... }
/// ```
declare_lint! {
    pub DEPRECATED_SEMVER, Warn,
    "`Warn` on `#[deprecated(since = \"x\")]` where x is not semver"
}

#[derive(Copy,Clone)]
pub struct AttrPass;

impl LintPass for AttrPass {
    fn get_lints(&self) -> LintArray {
        lint_array!(INLINE_ALWAYS, DEPRECATED_SEMVER)
    }
}

impl LateLintPass for AttrPass {
    fn check_attribute(&mut self, cx: &LateContext, attr: &Attribute) {
        if let MetaItemKind::List(ref name, ref items) = attr.node.value.node {
            if items.is_empty() || name != &"deprecated" {
                return;
            }
            for ref item in items {
                if let MetaItemKind::NameValue(ref name, ref lit) = item.node {
                    if name == &"since" {
                        check_semver(cx, item.span, lit);
                    }
                }
            }
        }
    }

    fn check_item(&mut self, cx: &LateContext, item: &Item) {
        if is_relevant_item(item) {
            check_attrs(cx, item.span, &item.name, &item.attrs)
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext, item: &ImplItem) {
        if is_relevant_impl(item) {
            check_attrs(cx, item.span, &item.name, &item.attrs)
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext, item: &TraitItem) {
        if is_relevant_trait(item) {
            check_attrs(cx, item.span, &item.name, &item.attrs)
        }
    }
}

fn is_relevant_item(item: &Item) -> bool {
    if let ItemFn(_, _, _, _, _, ref block) = item.node {
        is_relevant_block(block)
    } else {
        false
    }
}

fn is_relevant_impl(item: &ImplItem) -> bool {
    match item.node {
        ImplItemKind::Method(_, ref block) => is_relevant_block(block),
        _ => false,
    }
}

fn is_relevant_trait(item: &TraitItem) -> bool {
    match item.node {
        MethodTraitItem(_, None) => true,
        MethodTraitItem(_, Some(ref block)) => is_relevant_block(block),
        _ => false,
    }
}

fn is_relevant_block(block: &Block) -> bool {
    for stmt in &block.stmts {
        match stmt.node {
            StmtDecl(_, _) => return true,
            StmtExpr(ref expr, _) |
            StmtSemi(ref expr, _) => {
                return is_relevant_expr(expr);
            }
        }
    }
    block.expr.as_ref().map_or(false, |e| is_relevant_expr(e))
}

fn is_relevant_expr(expr: &Expr) -> bool {
    match expr.node {
        ExprBlock(ref block) => is_relevant_block(block),
        ExprRet(Some(ref e)) => is_relevant_expr(e),
        ExprRet(None) | ExprBreak(_) => false,
        ExprCall(ref path_expr, _) => {
            if let ExprPath(_, ref path) = path_expr.node {
                !match_path(path, &paths::BEGIN_UNWIND)
            } else {
                true
            }
        }
        _ => true,
    }
}

fn check_attrs(cx: &LateContext, span: Span, name: &Name, attrs: &[Attribute]) {
    if in_macro(cx, span) {
        return;
    }

    for attr in attrs {
        if let MetaItemKind::List(ref inline, ref values) = attr.node.value.node {
            if values.len() != 1 || inline != &"inline" {
                continue;
            }
            if let MetaItemKind::Word(ref always) = values[0].node {
                if always != &"always" {
                    continue;
                }
                span_lint(cx,
                          INLINE_ALWAYS,
                          attr.span,
                          &format!("you have declared `#[inline(always)]` on `{}`. This is usually a bad idea",
                                   name));
            }
        }
    }
}

fn check_semver(cx: &LateContext, span: Span, lit: &Lit) {
    if let LitKind::Str(ref is, _) = lit.node {
        if Version::parse(&*is).is_ok() {
            return;
        }
    }
    span_lint(cx,
              DEPRECATED_SEMVER,
              span,
              "the since field must contain a semver-compliant version");
}
