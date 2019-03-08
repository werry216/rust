use crate::reexport::*;
use if_chain::if_chain;
use matches::matches;
use rustc::hir;
use rustc::hir::def::Def;
use rustc::hir::def_id::{DefId, CRATE_DEF_INDEX, LOCAL_CRATE};
use rustc::hir::intravisit::{NestedVisitorMap, Visitor};
use rustc::hir::Node;
use rustc::hir::*;
use rustc::lint::{LateContext, Level, Lint, LintContext};
use rustc::traits;
use rustc::ty::{
    self,
    layout::{self, IntegerExt},
    subst::Kind,
    Binder, Ty, TyCtxt,
};
use rustc_data_structures::sync::Lrc;
use rustc_errors::Applicability;
use std::borrow::Cow;
use std::mem;
use syntax::ast::{self, LitKind};
use syntax::attr;
use syntax::source_map::{Span, DUMMY_SP};
use syntax::symbol;
use syntax::symbol::{keywords, Symbol};

pub mod attrs;
pub mod author;
pub mod camel_case;
pub mod comparisons;
pub mod conf;
pub mod constants;
mod diagnostics;
pub mod higher;
mod hir_utils;
pub mod inspector;
pub mod internal_lints;
pub mod paths;
pub mod ptr;
pub mod sugg;
pub mod usage;
pub use self::attrs::*;
pub use self::diagnostics::*;
pub use self::hir_utils::{SpanlessEq, SpanlessHash};

/// Returns true if the two spans come from differing expansions (i.e. one is
/// from a macro and one
/// isn't).
pub fn differing_macro_contexts(lhs: Span, rhs: Span) -> bool {
    rhs.ctxt() != lhs.ctxt()
}

/// Returns `true` if the given `NodeId` is inside a constant context
///
/// # Example
///
/// ```rust,ignore
/// if in_constant(cx, expr.id) {
///     // Do something
/// }
/// ```
pub fn in_constant(cx: &LateContext<'_, '_>, id: HirId) -> bool {
    let parent_id = cx.tcx.hir().get_parent_item(id);
    match cx.tcx.hir().get_by_hir_id(parent_id) {
        Node::Item(&Item {
            node: ItemKind::Const(..),
            ..
        })
        | Node::TraitItem(&TraitItem {
            node: TraitItemKind::Const(..),
            ..
        })
        | Node::ImplItem(&ImplItem {
            node: ImplItemKind::Const(..),
            ..
        })
        | Node::AnonConst(_)
        | Node::Item(&Item {
            node: ItemKind::Static(..),
            ..
        }) => true,
        Node::Item(&Item {
            node: ItemKind::Fn(_, header, ..),
            ..
        }) => header.constness == Constness::Const,
        _ => false,
    }
}

/// Returns true if this `expn_info` was expanded by any macro.
pub fn in_macro(span: Span) -> bool {
    span.ctxt().outer().expn_info().is_some()
}

/// Used to store the absolute path to a type.
///
/// See `match_def_path` for usage.
#[derive(Debug)]
pub struct AbsolutePathBuffer {
    pub names: Vec<symbol::LocalInternedString>,
}

impl ty::item_path::ItemPathBuffer for AbsolutePathBuffer {
    fn root_mode(&self) -> &ty::item_path::RootMode {
        const ABSOLUTE: &ty::item_path::RootMode = &ty::item_path::RootMode::Absolute;
        ABSOLUTE
    }

    fn push(&mut self, text: &str) {
        self.names.push(symbol::Symbol::intern(text).as_str());
    }
}

/// Check if a `DefId`'s path matches the given absolute type path usage.
///
/// # Examples
/// ```rust,ignore
/// match_def_path(cx.tcx, id, &["core", "option", "Option"])
/// ```
///
/// See also the `paths` module.
pub fn match_def_path(tcx: TyCtxt<'_, '_, '_>, def_id: DefId, path: &[&str]) -> bool {
    let mut apb = AbsolutePathBuffer { names: vec![] };

    tcx.push_item_path(&mut apb, def_id, false);

    apb.names.len() == path.len() && apb.names.into_iter().zip(path.iter()).all(|(a, &b)| *a == *b)
}

/// Get the absolute path of `def_id` as a vector of `&str`.
///
/// # Examples
/// ```rust,ignore
/// let def_path = get_def_path(tcx, def_id);
/// if let &["core", "option", "Option"] = &def_path[..] {
///     // The given `def_id` is that of an `Option` type
/// };
/// ```
pub fn get_def_path(tcx: TyCtxt<'_, '_, '_>, def_id: DefId) -> Vec<&'static str> {
    let mut apb = AbsolutePathBuffer { names: vec![] };
    tcx.push_item_path(&mut apb, def_id, false);
    apb.names
        .iter()
        .map(syntax_pos::symbol::LocalInternedString::get)
        .collect()
}

/// Check if type is struct, enum or union type with given def path.
pub fn match_type(cx: &LateContext<'_, '_>, ty: Ty<'_>, path: &[&str]) -> bool {
    match ty.sty {
        ty::Adt(adt, _) => match_def_path(cx.tcx, adt.did, path),
        _ => false,
    }
}

/// Check if the method call given in `expr` belongs to given trait.
pub fn match_trait_method(cx: &LateContext<'_, '_>, expr: &Expr, path: &[&str]) -> bool {
    let method_call = cx.tables.type_dependent_defs()[expr.hir_id];
    let trt_id = cx.tcx.trait_of_item(method_call.def_id());
    if let Some(trt_id) = trt_id {
        match_def_path(cx.tcx, trt_id, path)
    } else {
        false
    }
}

/// Check if an expression references a variable of the given name.
pub fn match_var(expr: &Expr, var: Name) -> bool {
    if let ExprKind::Path(QPath::Resolved(None, ref path)) = expr.node {
        if path.segments.len() == 1 && path.segments[0].ident.name == var {
            return true;
        }
    }
    false
}

pub fn last_path_segment(path: &QPath) -> &PathSegment {
    match *path {
        QPath::Resolved(_, ref path) => path.segments.last().expect("A path must have at least one segment"),
        QPath::TypeRelative(_, ref seg) => seg,
    }
}

pub fn single_segment_path(path: &QPath) -> Option<&PathSegment> {
    match *path {
        QPath::Resolved(_, ref path) if path.segments.len() == 1 => Some(&path.segments[0]),
        QPath::Resolved(..) => None,
        QPath::TypeRelative(_, ref seg) => Some(seg),
    }
}

/// Match a `Path` against a slice of segment string literals.
///
/// # Examples
/// ```rust,ignore
/// match_qpath(path, &["std", "rt", "begin_unwind"])
/// ```
pub fn match_qpath(path: &QPath, segments: &[&str]) -> bool {
    match *path {
        QPath::Resolved(_, ref path) => match_path(path, segments),
        QPath::TypeRelative(ref ty, ref segment) => match ty.node {
            TyKind::Path(ref inner_path) => {
                !segments.is_empty()
                    && match_qpath(inner_path, &segments[..(segments.len() - 1)])
                    && segment.ident.name == segments[segments.len() - 1]
            },
            _ => false,
        },
    }
}

pub fn match_path(path: &Path, segments: &[&str]) -> bool {
    path.segments
        .iter()
        .rev()
        .zip(segments.iter().rev())
        .all(|(a, b)| a.ident.name == *b)
}

/// Match a `Path` against a slice of segment string literals, e.g.
///
/// # Examples
/// ```rust,ignore
/// match_qpath(path, &["std", "rt", "begin_unwind"])
/// ```
pub fn match_path_ast(path: &ast::Path, segments: &[&str]) -> bool {
    path.segments
        .iter()
        .rev()
        .zip(segments.iter().rev())
        .all(|(a, b)| a.ident.name == *b)
}

/// Get the definition associated to a path.
pub fn path_to_def(cx: &LateContext<'_, '_>, path: &[&str]) -> Option<def::Def> {
    let crates = cx.tcx.crates();
    let krate = crates.iter().find(|&&krate| cx.tcx.crate_name(krate) == path[0]);
    if let Some(krate) = krate {
        let krate = DefId {
            krate: *krate,
            index: CRATE_DEF_INDEX,
        };
        let mut items = cx.tcx.item_children(krate);
        let mut path_it = path.iter().skip(1).peekable();

        loop {
            let segment = match path_it.next() {
                Some(segment) => segment,
                None => return None,
            };

            for item in mem::replace(&mut items, Lrc::new(vec![])).iter() {
                if item.ident.name == *segment {
                    if path_it.peek().is_none() {
                        return Some(item.def);
                    }

                    items = cx.tcx.item_children(item.def.def_id());
                    break;
                }
            }
        }
    } else {
        None
    }
}

/// Convenience function to get the `DefId` of a trait by path.
pub fn get_trait_def_id(cx: &LateContext<'_, '_>, path: &[&str]) -> Option<DefId> {
    let def = match path_to_def(cx, path) {
        Some(def) => def,
        None => return None,
    };

    match def {
        def::Def::Trait(trait_id) => Some(trait_id),
        _ => None,
    }
}

/// Check whether a type implements a trait.
/// See also `get_trait_def_id`.
pub fn implements_trait<'a, 'tcx>(
    cx: &LateContext<'a, 'tcx>,
    ty: Ty<'tcx>,
    trait_id: DefId,
    ty_params: &[Kind<'tcx>],
) -> bool {
    let ty = cx.tcx.erase_regions(&ty);
    let obligation = cx.tcx.predicate_for_trait_def(
        cx.param_env,
        traits::ObligationCause::dummy(),
        trait_id,
        0,
        ty,
        ty_params,
    );
    cx.tcx
        .infer_ctxt()
        .enter(|infcx| infcx.predicate_must_hold_modulo_regions(&obligation))
}

/// Get the `hir::TraitRef` of the trait the given method is implemented for
///
/// Use this if you want to find the `TraitRef` of the `Point` trait in this example:
///
/// ```rust
/// struct Point(isize, isize);
///
/// impl std::ops::Add for Point {
///     type Output = Self;
///
///     fn add(self, other: Self) -> Self {
///         Point(0, 0)
///     }
/// }
/// ```
pub fn trait_ref_of_method(cx: &LateContext<'_, '_>, hir_id: HirId) -> Option<TraitRef> {
    // Get the implemented trait for the current function
    let parent_impl = cx.tcx.hir().get_parent_item(hir_id);
    if_chain! {
        if parent_impl != hir::CRATE_HIR_ID;
        if let hir::Node::Item(item) = cx.tcx.hir().get_by_hir_id(parent_impl);
        if let hir::ItemKind::Impl(_, _, _, _, trait_ref, _, _) = &item.node;
        then { return trait_ref.clone(); }
    }
    None
}

/// Check whether this type implements Drop.
pub fn has_drop<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.ty_adt_def() {
        Some(def) => def.has_dtor(cx.tcx),
        _ => false,
    }
}

/// Resolve the definition of a node from its `HirId`.
pub fn resolve_node(cx: &LateContext<'_, '_>, qpath: &QPath, id: HirId) -> def::Def {
    cx.tables.qpath_def(qpath, id)
}

/// Return the method names and argument list of nested method call expressions that make up
/// `expr`.
pub fn method_calls<'a>(expr: &'a Expr, max_depth: usize) -> (Vec<Symbol>, Vec<&'a [Expr]>) {
    let mut method_names = Vec::with_capacity(max_depth);
    let mut arg_lists = Vec::with_capacity(max_depth);

    let mut current = expr;
    for _ in 0..max_depth {
        if let ExprKind::MethodCall(path, _, args) = &current.node {
            if args.iter().any(|e| in_macro(e.span)) {
                break;
            }
            method_names.push(path.ident.name);
            arg_lists.push(&**args);
            current = &args[0];
        } else {
            break;
        }
    }

    (method_names, arg_lists)
}

/// Match an `Expr` against a chain of methods, and return the matched `Expr`s.
///
/// For example, if `expr` represents the `.baz()` in `foo.bar().baz()`,
/// `matched_method_chain(expr, &["bar", "baz"])` will return a `Vec`
/// containing the `Expr`s for
/// `.bar()` and `.baz()`
pub fn method_chain_args<'a>(expr: &'a Expr, methods: &[&str]) -> Option<Vec<&'a [Expr]>> {
    let mut current = expr;
    let mut matched = Vec::with_capacity(methods.len());
    for method_name in methods.iter().rev() {
        // method chains are stored last -> first
        if let ExprKind::MethodCall(ref path, _, ref args) = current.node {
            if path.ident.name == *method_name {
                if args.iter().any(|e| in_macro(e.span)) {
                    return None;
                }
                matched.push(&**args); // build up `matched` backwards
                current = &args[0] // go to parent expression
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    matched.reverse(); // reverse `matched`, so that it is in the same order as `methods`
    Some(matched)
}

/// Returns true if the provided `def_id` is an entrypoint to a program
pub fn is_entrypoint_fn(cx: &LateContext<'_, '_>, def_id: DefId) -> bool {
    if let Some((entry_fn_def_id, _)) = cx.tcx.entry_fn(LOCAL_CRATE) {
        return def_id == entry_fn_def_id;
    }
    false
}

/// Get the name of the item the expression is in, if available.
pub fn get_item_name(cx: &LateContext<'_, '_>, expr: &Expr) -> Option<Name> {
    let parent_id = cx.tcx.hir().get_parent_item(expr.hir_id);
    match cx.tcx.hir().find_by_hir_id(parent_id) {
        Some(Node::Item(&Item { ref ident, .. })) => Some(ident.name),
        Some(Node::TraitItem(&TraitItem { ident, .. })) | Some(Node::ImplItem(&ImplItem { ident, .. })) => {
            Some(ident.name)
        },
        _ => None,
    }
}

/// Get the name of a `Pat`, if any
pub fn get_pat_name(pat: &Pat) -> Option<Name> {
    match pat.node {
        PatKind::Binding(.., ref spname, _) => Some(spname.name),
        PatKind::Path(ref qpath) => single_segment_path(qpath).map(|ps| ps.ident.name),
        PatKind::Box(ref p) | PatKind::Ref(ref p, _) => get_pat_name(&*p),
        _ => None,
    }
}

struct ContainsName {
    name: Name,
    result: bool,
}

impl<'tcx> Visitor<'tcx> for ContainsName {
    fn visit_name(&mut self, _: Span, name: Name) {
        if self.name == name {
            self.result = true;
        }
    }
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'tcx> {
        NestedVisitorMap::None
    }
}

/// check if an `Expr` contains a certain name
pub fn contains_name(name: Name, expr: &Expr) -> bool {
    let mut cn = ContainsName { name, result: false };
    cn.visit_expr(expr);
    cn.result
}

/// Convert a span to a code snippet if available, otherwise use default.
///
/// This is useful if you want to provide suggestions for your lint or more generally, if you want
/// to convert a given `Span` to a `str`.
///
/// # Example
/// ```rust,ignore
/// snippet(cx, expr.span, "..")
/// ```
pub fn snippet<'a, 'b, T: LintContext<'b>>(cx: &T, span: Span, default: &'a str) -> Cow<'a, str> {
    snippet_opt(cx, span).map_or_else(|| Cow::Borrowed(default), From::from)
}

/// Same as `snippet`, but it adapts the applicability level by following rules:
///
/// - Applicability level `Unspecified` will never be changed.
/// - If the span is inside a macro, change the applicability level to `MaybeIncorrect`.
/// - If the default value is used and the applicability level is `MachineApplicable`, change it to
/// `HasPlaceholders`
pub fn snippet_with_applicability<'a, 'b, T: LintContext<'b>>(
    cx: &T,
    span: Span,
    default: &'a str,
    applicability: &mut Applicability,
) -> Cow<'a, str> {
    if *applicability != Applicability::Unspecified && in_macro(span) {
        *applicability = Applicability::MaybeIncorrect;
    }
    snippet_opt(cx, span).map_or_else(
        || {
            if *applicability == Applicability::MachineApplicable {
                *applicability = Applicability::HasPlaceholders;
            }
            Cow::Borrowed(default)
        },
        From::from,
    )
}

/// Same as `snippet`, but should only be used when it's clear that the input span is
/// not a macro argument.
pub fn snippet_with_macro_callsite<'a, 'b, T: LintContext<'b>>(cx: &T, span: Span, default: &'a str) -> Cow<'a, str> {
    snippet(cx, span.source_callsite(), default)
}

/// Convert a span to a code snippet. Returns `None` if not available.
pub fn snippet_opt<'a, T: LintContext<'a>>(cx: &T, span: Span) -> Option<String> {
    cx.sess().source_map().span_to_snippet(span).ok()
}

/// Convert a span (from a block) to a code snippet if available, otherwise use
/// default.
/// This trims the code of indentation, except for the first line. Use it for
/// blocks or block-like
/// things which need to be printed as such.
///
/// # Example
/// ```rust,ignore
/// snippet_block(cx, expr.span, "..")
/// ```
pub fn snippet_block<'a, 'b, T: LintContext<'b>>(cx: &T, span: Span, default: &'a str) -> Cow<'a, str> {
    let snip = snippet(cx, span, default);
    trim_multiline(snip, true)
}

/// Same as `snippet_block`, but adapts the applicability level by the rules of
/// `snippet_with_applicabiliy`.
pub fn snippet_block_with_applicability<'a, 'b, T: LintContext<'b>>(
    cx: &T,
    span: Span,
    default: &'a str,
    applicability: &mut Applicability,
) -> Cow<'a, str> {
    let snip = snippet_with_applicability(cx, span, default, applicability);
    trim_multiline(snip, true)
}

/// Returns a new Span that covers the full last line of the given Span
pub fn last_line_of_span<'a, T: LintContext<'a>>(cx: &T, span: Span) -> Span {
    let source_map_and_line = cx.sess().source_map().lookup_line(span.lo()).unwrap();
    let line_no = source_map_and_line.line;
    let line_start = &source_map_and_line.sf.lines[line_no];
    Span::new(*line_start, span.hi(), span.ctxt())
}

/// Like `snippet_block`, but add braces if the expr is not an `ExprKind::Block`.
/// Also takes an `Option<String>` which can be put inside the braces.
pub fn expr_block<'a, 'b, T: LintContext<'b>>(
    cx: &T,
    expr: &Expr,
    option: Option<String>,
    default: &'a str,
) -> Cow<'a, str> {
    let code = snippet_block(cx, expr.span, default);
    let string = option.unwrap_or_default();
    if in_macro(expr.span) {
        Cow::Owned(format!("{{ {} }}", snippet_with_macro_callsite(cx, expr.span, default)))
    } else if let ExprKind::Block(_, _) = expr.node {
        Cow::Owned(format!("{}{}", code, string))
    } else if string.is_empty() {
        Cow::Owned(format!("{{ {} }}", code))
    } else {
        Cow::Owned(format!("{{\n{};\n{}\n}}", code, string))
    }
}

/// Trim indentation from a multiline string with possibility of ignoring the
/// first line.
pub fn trim_multiline(s: Cow<'_, str>, ignore_first: bool) -> Cow<'_, str> {
    let s_space = trim_multiline_inner(s, ignore_first, ' ');
    let s_tab = trim_multiline_inner(s_space, ignore_first, '\t');
    trim_multiline_inner(s_tab, ignore_first, ' ')
}

fn trim_multiline_inner(s: Cow<'_, str>, ignore_first: bool, ch: char) -> Cow<'_, str> {
    let x = s
        .lines()
        .skip(ignore_first as usize)
        .filter_map(|l| {
            if l.is_empty() {
                None
            } else {
                // ignore empty lines
                Some(l.char_indices().find(|&(_, x)| x != ch).unwrap_or((l.len(), ch)).0)
            }
        })
        .min()
        .unwrap_or(0);
    if x > 0 {
        Cow::Owned(
            s.lines()
                .enumerate()
                .map(|(i, l)| {
                    if (ignore_first && i == 0) || l.is_empty() {
                        l
                    } else {
                        l.split_at(x).1
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        s
    }
}

/// Get a parent expressions if any – this is useful to constrain a lint.
pub fn get_parent_expr<'c>(cx: &'c LateContext<'_, '_>, e: &Expr) -> Option<&'c Expr> {
    let map = &cx.tcx.hir();
    let hir_id = e.hir_id;
    let parent_id = map.get_parent_node_by_hir_id(hir_id);
    if hir_id == parent_id {
        return None;
    }
    map.find_by_hir_id(parent_id).and_then(|node| {
        if let Node::Expr(parent) = node {
            Some(parent)
        } else {
            None
        }
    })
}

pub fn get_enclosing_block<'a, 'tcx: 'a>(cx: &LateContext<'a, 'tcx>, node: HirId) -> Option<&'tcx Block> {
    let map = &cx.tcx.hir();
    let node_id = map.hir_to_node_id(node);
    let enclosing_node = map
        .get_enclosing_scope(node_id)
        .and_then(|enclosing_id| map.find(enclosing_id));
    if let Some(node) = enclosing_node {
        match node {
            Node::Block(block) => Some(block),
            Node::Item(&Item {
                node: ItemKind::Fn(_, _, _, eid),
                ..
            })
            | Node::ImplItem(&ImplItem {
                node: ImplItemKind::Method(_, eid),
                ..
            }) => match cx.tcx.hir().body(eid).value.node {
                ExprKind::Block(ref block, _) => Some(block),
                _ => None,
            },
            _ => None,
        }
    } else {
        None
    }
}

/// Return the base type for HIR references and pointers.
pub fn walk_ptrs_hir_ty(ty: &hir::Ty) -> &hir::Ty {
    match ty.node {
        TyKind::Ptr(ref mut_ty) | TyKind::Rptr(_, ref mut_ty) => walk_ptrs_hir_ty(&mut_ty.ty),
        _ => ty,
    }
}

/// Return the base type for references and raw pointers.
pub fn walk_ptrs_ty(ty: Ty<'_>) -> Ty<'_> {
    match ty.sty {
        ty::Ref(_, ty, _) => walk_ptrs_ty(ty),
        _ => ty,
    }
}

/// Return the base type for references and raw pointers, and count reference
/// depth.
pub fn walk_ptrs_ty_depth(ty: Ty<'_>) -> (Ty<'_>, usize) {
    fn inner(ty: Ty<'_>, depth: usize) -> (Ty<'_>, usize) {
        match ty.sty {
            ty::Ref(_, ty, _) => inner(ty, depth + 1),
            _ => (ty, depth),
        }
    }
    inner(ty, 0)
}

/// Check whether the given expression is a constant literal of the given value.
pub fn is_integer_literal(expr: &Expr, value: u128) -> bool {
    // FIXME: use constant folding
    if let ExprKind::Lit(ref spanned) = expr.node {
        if let LitKind::Int(v, _) = spanned.node {
            return v == value;
        }
    }
    false
}

/// Returns `true` if the given `Expr` has been coerced before.
///
/// Examples of coercions can be found in the Nomicon at
/// <https://doc.rust-lang.org/nomicon/coercions.html>.
///
/// See `rustc::ty::adjustment::Adjustment` and `rustc_typeck::check::coercion` for more
/// information on adjustments and coercions.
pub fn is_adjusted(cx: &LateContext<'_, '_>, e: &Expr) -> bool {
    cx.tables.adjustments().get(e.hir_id).is_some()
}

/// Return the pre-expansion span if is this comes from an expansion of the
/// macro `name`.
/// See also `is_direct_expn_of`.
pub fn is_expn_of(mut span: Span, name: &str) -> Option<Span> {
    loop {
        let span_name_span = span
            .ctxt()
            .outer()
            .expn_info()
            .map(|ei| (ei.format.name(), ei.call_site));

        match span_name_span {
            Some((mac_name, new_span)) if mac_name == name => return Some(new_span),
            None => return None,
            Some((_, new_span)) => span = new_span,
        }
    }
}

/// Return the pre-expansion span if is this directly comes from an expansion
/// of the macro `name`.
/// The difference with `is_expn_of` is that in
/// ```rust,ignore
/// foo!(bar!(42));
/// ```
/// `42` is considered expanded from `foo!` and `bar!` by `is_expn_of` but only
/// `bar!` by
/// `is_direct_expn_of`.
pub fn is_direct_expn_of(span: Span, name: &str) -> Option<Span> {
    let span_name_span = span
        .ctxt()
        .outer()
        .expn_info()
        .map(|ei| (ei.format.name(), ei.call_site));

    match span_name_span {
        Some((mac_name, new_span)) if mac_name == name => Some(new_span),
        _ => None,
    }
}

/// Convenience function to get the return type of a function
pub fn return_ty<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, fn_item: hir::HirId) -> Ty<'tcx> {
    let fn_def_id = cx.tcx.hir().local_def_id_from_hir_id(fn_item);
    let ret_ty = cx.tcx.fn_sig(fn_def_id).output();
    cx.tcx.erase_late_bound_regions(&ret_ty)
}

/// Check if two types are the same.
///
/// This discards any lifetime annotations, too.
// FIXME: this works correctly for lifetimes bounds (`for <'a> Foo<'a>` == `for
// <'b> Foo<'b>` but
// not for type parameters.
pub fn same_tys<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, a: Ty<'tcx>, b: Ty<'tcx>) -> bool {
    let a = cx.tcx.erase_late_bound_regions(&Binder::bind(a));
    let b = cx.tcx.erase_late_bound_regions(&Binder::bind(b));
    cx.tcx
        .infer_ctxt()
        .enter(|infcx| infcx.can_eq(cx.param_env, a, b).is_ok())
}

/// Return whether the given type is an `unsafe` function.
pub fn type_is_unsafe_function<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.sty {
        ty::FnDef(..) | ty::FnPtr(_) => ty.fn_sig(cx.tcx).unsafety() == Unsafety::Unsafe,
        _ => false,
    }
}

pub fn is_copy<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, ty: Ty<'tcx>) -> bool {
    ty.is_copy_modulo_regions(cx.tcx.global_tcx(), cx.param_env, DUMMY_SP)
}

/// Return whether a pattern is refutable.
pub fn is_refutable(cx: &LateContext<'_, '_>, pat: &Pat) -> bool {
    fn is_enum_variant(cx: &LateContext<'_, '_>, qpath: &QPath, id: HirId) -> bool {
        matches!(
            cx.tables.qpath_def(qpath, id),
            def::Def::Variant(..) | def::Def::VariantCtor(..)
        )
    }

    fn are_refutable<'a, I: Iterator<Item = &'a Pat>>(cx: &LateContext<'_, '_>, mut i: I) -> bool {
        i.any(|pat| is_refutable(cx, pat))
    }

    match pat.node {
        PatKind::Binding(..) | PatKind::Wild => false,
        PatKind::Box(ref pat) | PatKind::Ref(ref pat, _) => is_refutable(cx, pat),
        PatKind::Lit(..) | PatKind::Range(..) => true,
        PatKind::Path(ref qpath) => is_enum_variant(cx, qpath, pat.hir_id),
        PatKind::Tuple(ref pats, _) => are_refutable(cx, pats.iter().map(|pat| &**pat)),
        PatKind::Struct(ref qpath, ref fields, _) => {
            if is_enum_variant(cx, qpath, pat.hir_id) {
                true
            } else {
                are_refutable(cx, fields.iter().map(|field| &*field.node.pat))
            }
        },
        PatKind::TupleStruct(ref qpath, ref pats, _) => {
            if is_enum_variant(cx, qpath, pat.hir_id) {
                true
            } else {
                are_refutable(cx, pats.iter().map(|pat| &**pat))
            }
        },
        PatKind::Slice(ref head, ref middle, ref tail) => {
            are_refutable(cx, head.iter().chain(middle).chain(tail.iter()).map(|pat| &**pat))
        },
    }
}

/// Checks for the `#[automatically_derived]` attribute all `#[derive]`d
/// implementations have.
pub fn is_automatically_derived(attrs: &[ast::Attribute]) -> bool {
    attr::contains_name(attrs, "automatically_derived")
}

/// Remove blocks around an expression.
///
/// Ie. `x`, `{ x }` and `{{{{ x }}}}` all give `x`. `{ x; y }` and `{}` return
/// themselves.
pub fn remove_blocks(expr: &Expr) -> &Expr {
    if let ExprKind::Block(ref block, _) = expr.node {
        if block.stmts.is_empty() {
            if let Some(ref expr) = block.expr {
                remove_blocks(expr)
            } else {
                expr
            }
        } else {
            expr
        }
    } else {
        expr
    }
}

pub fn opt_def_id(def: Def) -> Option<DefId> {
    def.opt_def_id()
}

pub fn is_self(slf: &Arg) -> bool {
    if let PatKind::Binding(.., name, _) = slf.pat.node {
        name.name == keywords::SelfLower.name()
    } else {
        false
    }
}

pub fn is_self_ty(slf: &hir::Ty) -> bool {
    if_chain! {
        if let TyKind::Path(ref qp) = slf.node;
        if let QPath::Resolved(None, ref path) = *qp;
        if let Def::SelfTy(..) = path.def;
        then {
            return true
        }
    }
    false
}

pub fn iter_input_pats<'tcx>(decl: &FnDecl, body: &'tcx Body) -> impl Iterator<Item = &'tcx Arg> {
    (0..decl.inputs.len()).map(move |i| &body.arguments[i])
}

/// Check if a given expression is a match expression
/// expanded from `?` operator or `try` macro.
pub fn is_try(expr: &Expr) -> Option<&Expr> {
    fn is_ok(arm: &Arm) -> bool {
        if_chain! {
            if let PatKind::TupleStruct(ref path, ref pat, None) = arm.pats[0].node;
            if match_qpath(path, &paths::RESULT_OK[1..]);
            if let PatKind::Binding(_, defid, _, _, None) = pat[0].node;
            if let ExprKind::Path(QPath::Resolved(None, ref path)) = arm.body.node;
            if let Def::Local(lid) = path.def;
            if lid == defid;
            then {
                return true;
            }
        }
        false
    }

    fn is_err(arm: &Arm) -> bool {
        if let PatKind::TupleStruct(ref path, _, _) = arm.pats[0].node {
            match_qpath(path, &paths::RESULT_ERR[1..])
        } else {
            false
        }
    }

    if let ExprKind::Match(_, ref arms, ref source) = expr.node {
        // desugared from a `?` operator
        if let MatchSource::TryDesugar = *source {
            return Some(expr);
        }

        if_chain! {
            if arms.len() == 2;
            if arms[0].pats.len() == 1 && arms[0].guard.is_none();
            if arms[1].pats.len() == 1 && arms[1].guard.is_none();
            if (is_ok(&arms[0]) && is_err(&arms[1])) ||
                (is_ok(&arms[1]) && is_err(&arms[0]));
            then {
                return Some(expr);
            }
        }
    }

    None
}

/// Returns true if the lint is allowed in the current context
///
/// Useful for skipping long running code when it's unnecessary
pub fn is_allowed(cx: &LateContext<'_, '_>, lint: &'static Lint, id: HirId) -> bool {
    cx.tcx.lint_level_at_node(lint, id).0 == Level::Allow
}

pub fn get_arg_name(pat: &Pat) -> Option<ast::Name> {
    match pat.node {
        PatKind::Binding(.., ident, None) => Some(ident.name),
        PatKind::Ref(ref subpat, _) => get_arg_name(subpat),
        _ => None,
    }
}

pub fn int_bits(tcx: TyCtxt<'_, '_, '_>, ity: ast::IntTy) -> u64 {
    layout::Integer::from_attr(&tcx, attr::IntType::SignedInt(ity))
        .size()
        .bits()
}

#[allow(clippy::cast_possible_wrap)]
/// Turn a constant int byte representation into an i128
pub fn sext(tcx: TyCtxt<'_, '_, '_>, u: u128, ity: ast::IntTy) -> i128 {
    let amt = 128 - int_bits(tcx, ity);
    ((u as i128) << amt) >> amt
}

#[allow(clippy::cast_sign_loss)]
/// clip unused bytes
pub fn unsext(tcx: TyCtxt<'_, '_, '_>, u: i128, ity: ast::IntTy) -> u128 {
    let amt = 128 - int_bits(tcx, ity);
    ((u as u128) << amt) >> amt
}

/// clip unused bytes
pub fn clip(tcx: TyCtxt<'_, '_, '_>, u: u128, ity: ast::UintTy) -> u128 {
    let bits = layout::Integer::from_attr(&tcx, attr::IntType::UnsignedInt(ity))
        .size()
        .bits();
    let amt = 128 - bits;
    (u << amt) >> amt
}

/// Remove block comments from the given Vec of lines
///
/// # Examples
///
/// ```rust,ignore
/// without_block_comments(vec!["/*", "foo", "*/"]);
/// // => vec![]
///
/// without_block_comments(vec!["bar", "/*", "foo", "*/"]);
/// // => vec!["bar"]
/// ```
pub fn without_block_comments(lines: Vec<&str>) -> Vec<&str> {
    let mut without = vec![];

    let mut nest_level = 0;

    for line in lines {
        if line.contains("/*") {
            nest_level += 1;
            continue;
        } else if line.contains("*/") {
            nest_level -= 1;
            continue;
        }

        if nest_level == 0 {
            without.push(line);
        }
    }

    without
}

pub fn any_parent_is_automatically_derived(tcx: TyCtxt<'_, '_, '_>, node: HirId) -> bool {
    let map = &tcx.hir();
    let mut prev_enclosing_node = None;
    let mut enclosing_node = node;
    while Some(enclosing_node) != prev_enclosing_node {
        if is_automatically_derived(map.attrs_by_hir_id(enclosing_node)) {
            return true;
        }
        prev_enclosing_node = Some(enclosing_node);
        enclosing_node = map.get_parent_item(enclosing_node);
    }
    false
}

/// Returns true if ty has `iter` or `iter_mut` methods
pub fn has_iter_method(cx: &LateContext<'_, '_>, probably_ref_ty: ty::Ty<'_>) -> Option<&'static str> {
    // FIXME: instead of this hard-coded list, we should check if `<adt>::iter`
    // exists and has the desired signature. Unfortunately FnCtxt is not exported
    // so we can't use its `lookup_method` method.
    static INTO_ITER_COLLECTIONS: [&[&str]; 13] = [
        &paths::VEC,
        &paths::OPTION,
        &paths::RESULT,
        &paths::BTREESET,
        &paths::BTREEMAP,
        &paths::VEC_DEQUE,
        &paths::LINKED_LIST,
        &paths::BINARY_HEAP,
        &paths::HASHSET,
        &paths::HASHMAP,
        &paths::PATH_BUF,
        &paths::PATH,
        &paths::RECEIVER,
    ];

    let ty_to_check = match probably_ref_ty.sty {
        ty::Ref(_, ty_to_check, _) => ty_to_check,
        _ => probably_ref_ty,
    };

    let def_id = match ty_to_check.sty {
        ty::Array(..) => return Some("array"),
        ty::Slice(..) => return Some("slice"),
        ty::Adt(adt, _) => adt.did,
        _ => return None,
    };

    for path in &INTO_ITER_COLLECTIONS {
        if match_def_path(cx.tcx, def_id, path) {
            return Some(path.last().unwrap());
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::{trim_multiline, without_block_comments};

    #[test]
    fn test_trim_multiline_single_line() {
        assert_eq!("", trim_multiline("".into(), false));
        assert_eq!("...", trim_multiline("...".into(), false));
        assert_eq!("...", trim_multiline("    ...".into(), false));
        assert_eq!("...", trim_multiline("\t...".into(), false));
        assert_eq!("...", trim_multiline("\t\t...".into(), false));
    }

    #[test]
    #[rustfmt::skip]
    fn test_trim_multiline_block() {
        assert_eq!("\
    if x {
        y
    } else {
        z
    }", trim_multiline("    if x {
            y
        } else {
            z
        }".into(), false));
        assert_eq!("\
    if x {
    \ty
    } else {
    \tz
    }", trim_multiline("    if x {
        \ty
        } else {
        \tz
        }".into(), false));
    }

    #[test]
    #[rustfmt::skip]
    fn test_trim_multiline_empty_line() {
        assert_eq!("\
    if x {
        y

    } else {
        z
    }", trim_multiline("    if x {
            y

        } else {
            z
        }".into(), false));
    }

    #[test]
    fn test_without_block_comments_lines_without_block_comments() {
        let result = without_block_comments(vec!["/*", "", "*/"]);
        println!("result: {:?}", result);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["", "/*", "", "*/", "#[crate_type = \"lib\"]", "/*", "", "*/", ""]);
        assert_eq!(result, vec!["", "#[crate_type = \"lib\"]", ""]);

        let result = without_block_comments(vec!["/* rust", "", "*/"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* one-line comment */"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* nested", "/* multi-line", "comment", "*/", "test", "*/"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["/* nested /* inline /* comment */ test */ */"]);
        assert!(result.is_empty());

        let result = without_block_comments(vec!["foo", "bar", "baz"]);
        assert_eq!(result, vec!["foo", "bar", "baz"]);
    }
}
