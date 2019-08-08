// ignore-tidy-filelength

#![doc(html_root_url = "https://doc.rust-lang.org/nightly/")]

#![feature(crate_visibility_modifier)]
#![feature(label_break_value)]
#![feature(mem_take)]
#![feature(nll)]
#![feature(rustc_diagnostic_macros)]

#![recursion_limit="256"]

pub use rustc::hir::def::{Namespace, PerNS};

use Determinacy::*;
use RibKind::*;

use rustc::hir::map::Definitions;
use rustc::hir::{self, PrimTy, Bool, Char, Float, Int, Uint, Str};
use rustc::middle::cstore::CrateStore;
use rustc::session::Session;
use rustc::lint;
use rustc::hir::def::{
    self, DefKind, PartialRes, CtorKind, CtorOf, NonMacroAttrKind, ExportMap
};
use rustc::hir::def::Namespace::*;
use rustc::hir::def_id::{CRATE_DEF_INDEX, LOCAL_CRATE, DefId};
use rustc::hir::{TraitMap, GlobMap};
use rustc::ty;
use rustc::util::nodemap::{NodeMap, NodeSet, FxHashMap, FxHashSet, DefIdMap};
use rustc::{bug, span_bug};

use rustc_metadata::creader::CrateLoader;
use rustc_metadata::cstore::CStore;

use syntax::source_map::SourceMap;
use syntax::ext::hygiene::{ExpnId, Transparency, SyntaxContext};
use syntax::ast::{self, Name, NodeId, Ident, FloatTy, IntTy, UintTy};
use syntax::ext::base::{SyntaxExtension, MacroKind, SpecialDerives};
use syntax::symbol::{Symbol, kw, sym};

use syntax::visit::{self, Visitor};
use syntax::attr;
use syntax::ast::{CRATE_NODE_ID, Crate, Expr, ExprKind};
use syntax::ast::{ItemKind, Path};
use syntax::{span_err, struct_span_err, unwrap_or};

use syntax_pos::{Span, DUMMY_SP, MultiSpan};
use errors::{Applicability, DiagnosticBuilder, DiagnosticId};

use log::debug;

use std::cell::{Cell, RefCell};
use std::{cmp, fmt, iter, ptr};
use std::collections::BTreeSet;
use rustc_data_structures::ptr_key::PtrKey;
use rustc_data_structures::sync::Lrc;

use diagnostics::{Suggestion, ImportSuggestion};
use diagnostics::{find_span_of_binding_until_next_binding, extend_span_to_previous_binding};
use resolve_imports::{ImportDirective, ImportDirectiveSubclass, NameResolution, ImportResolver};
use macros::{InvocationData, LegacyBinding, LegacyScope};

type Res = def::Res<NodeId>;

// N.B., this module needs to be declared first so diagnostics are
// registered before they are used.
mod error_codes;
mod diagnostics;
mod late;
mod macros;
mod check_unused;
mod build_reduced_graph;
mod resolve_imports;

const KNOWN_TOOLS: &[Name] = &[sym::clippy, sym::rustfmt];

enum Weak {
    Yes,
    No,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Determinacy {
    Determined,
    Undetermined,
}

impl Determinacy {
    fn determined(determined: bool) -> Determinacy {
        if determined { Determinacy::Determined } else { Determinacy::Undetermined }
    }
}

/// A specific scope in which a name can be looked up.
/// This enum is currently used only for early resolution (imports and macros),
/// but not for late resolution yet.
#[derive(Clone, Copy)]
enum Scope<'a> {
    DeriveHelpers,
    MacroRules(LegacyScope<'a>),
    CrateRoot,
    Module(Module<'a>),
    MacroUsePrelude,
    BuiltinAttrs,
    LegacyPluginHelpers,
    ExternPrelude,
    ToolPrelude,
    StdLibPrelude,
    BuiltinTypes,
}

/// Names from different contexts may want to visit different subsets of all specific scopes
/// with different restrictions when looking up the resolution.
/// This enum is currently used only for early resolution (imports and macros),
/// but not for late resolution yet.
enum ScopeSet {
    Import(Namespace),
    AbsolutePath(Namespace),
    Macro(MacroKind),
    Module,
}

/// Everything you need to know about a name's location to resolve it.
/// Serves as a starting point for the scope visitor.
/// This struct is currently used only for early resolution (imports and macros),
/// but not for late resolution yet.
#[derive(Clone, Debug)]
pub struct ParentScope<'a> {
    module: Module<'a>,
    expansion: ExpnId,
    legacy: LegacyScope<'a>,
    derives: Vec<ast::Path>,
}

#[derive(Eq)]
struct BindingError {
    name: Name,
    origin: BTreeSet<Span>,
    target: BTreeSet<Span>,
}

impl PartialOrd for BindingError {
    fn partial_cmp(&self, other: &BindingError) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for BindingError {
    fn eq(&self, other: &BindingError) -> bool {
        self.name == other.name
    }
}

impl Ord for BindingError {
    fn cmp(&self, other: &BindingError) -> cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

enum ResolutionError<'a> {
    /// Error E0401: can't use type or const parameters from outer function.
    GenericParamsFromOuterFunction(Res),
    /// Error E0403: the name is already used for a type or const parameter in this generic
    /// parameter list.
    NameAlreadyUsedInParameterList(Name, Span),
    /// Error E0407: method is not a member of trait.
    MethodNotMemberOfTrait(Name, &'a str),
    /// Error E0437: type is not a member of trait.
    TypeNotMemberOfTrait(Name, &'a str),
    /// Error E0438: const is not a member of trait.
    ConstNotMemberOfTrait(Name, &'a str),
    /// Error E0408: variable `{}` is not bound in all patterns.
    VariableNotBoundInPattern(&'a BindingError),
    /// Error E0409: variable `{}` is bound in inconsistent ways within the same match arm.
    VariableBoundWithDifferentMode(Name, Span),
    /// Error E0415: identifier is bound more than once in this parameter list.
    IdentifierBoundMoreThanOnceInParameterList(&'a str),
    /// Error E0416: identifier is bound more than once in the same pattern.
    IdentifierBoundMoreThanOnceInSamePattern(&'a str),
    /// Error E0426: use of undeclared label.
    UndeclaredLabel(&'a str, Option<Name>),
    /// Error E0429: `self` imports are only allowed within a `{ }` list.
    SelfImportsOnlyAllowedWithin,
    /// Error E0430: `self` import can only appear once in the list.
    SelfImportCanOnlyAppearOnceInTheList,
    /// Error E0431: `self` import can only appear in an import list with a non-empty prefix.
    SelfImportOnlyInImportListWithNonEmptyPrefix,
    /// Error E0433: failed to resolve.
    FailedToResolve { label: String, suggestion: Option<Suggestion> },
    /// Error E0434: can't capture dynamic environment in a fn item.
    CannotCaptureDynamicEnvironmentInFnItem,
    /// Error E0435: attempt to use a non-constant value in a constant.
    AttemptToUseNonConstantValueInConstant,
    /// Error E0530: `X` bindings cannot shadow `Y`s.
    BindingShadowsSomethingUnacceptable(&'a str, Name, &'a NameBinding<'a>),
    /// Error E0128: type parameters with a default cannot use forward-declared identifiers.
    ForwardDeclaredTyParam, // FIXME(const_generics:defaults)
    /// Error E0671: const parameter cannot depend on type parameter.
    ConstParamDependentOnTypeParam,
}

/// Combines an error with provided span and emits it.
///
/// This takes the error provided, combines it with the span and any additional spans inside the
/// error and emits it.
fn resolve_error(resolver: &Resolver<'_>,
                 span: Span,
                 resolution_error: ResolutionError<'_>) {
    resolve_struct_error(resolver, span, resolution_error).emit();
}

fn resolve_struct_error<'sess, 'a>(resolver: &'sess Resolver<'_>,
                                   span: Span,
                                   resolution_error: ResolutionError<'a>)
                                   -> DiagnosticBuilder<'sess> {
    match resolution_error {
        ResolutionError::GenericParamsFromOuterFunction(outer_res) => {
            let mut err = struct_span_err!(resolver.session,
                span,
                E0401,
                "can't use generic parameters from outer function",
            );
            err.span_label(span, format!("use of generic parameter from outer function"));

            let cm = resolver.session.source_map();
            match outer_res {
                Res::SelfTy(maybe_trait_defid, maybe_impl_defid) => {
                    if let Some(impl_span) = maybe_impl_defid.and_then(|def_id| {
                        resolver.definitions.opt_span(def_id)
                    }) {
                        err.span_label(
                            reduce_impl_span_to_impl_keyword(cm, impl_span),
                            "`Self` type implicitly declared here, by this `impl`",
                        );
                    }
                    match (maybe_trait_defid, maybe_impl_defid) {
                        (Some(_), None) => {
                            err.span_label(span, "can't use `Self` here");
                        }
                        (_, Some(_)) => {
                            err.span_label(span, "use a type here instead");
                        }
                        (None, None) => bug!("`impl` without trait nor type?"),
                    }
                    return err;
                },
                Res::Def(DefKind::TyParam, def_id) => {
                    if let Some(span) = resolver.definitions.opt_span(def_id) {
                        err.span_label(span, "type parameter from outer function");
                    }
                }
                Res::Def(DefKind::ConstParam, def_id) => {
                    if let Some(span) = resolver.definitions.opt_span(def_id) {
                        err.span_label(span, "const parameter from outer function");
                    }
                }
                _ => {
                    bug!("GenericParamsFromOuterFunction should only be used with Res::SelfTy, \
                         DefKind::TyParam");
                }
            }

            // Try to retrieve the span of the function signature and generate a new message with
            // a local type or const parameter.
            let sugg_msg = &format!("try using a local generic parameter instead");
            if let Some((sugg_span, new_snippet)) = cm.generate_local_type_param_snippet(span) {
                // Suggest the modification to the user
                err.span_suggestion(
                    sugg_span,
                    sugg_msg,
                    new_snippet,
                    Applicability::MachineApplicable,
                );
            } else if let Some(sp) = cm.generate_fn_name_span(span) {
                err.span_label(sp,
                    format!("try adding a local generic parameter in this method instead"));
            } else {
                err.help(&format!("try using a local generic parameter instead"));
            }

            err
        }
        ResolutionError::NameAlreadyUsedInParameterList(name, first_use_span) => {
             let mut err = struct_span_err!(resolver.session,
                                            span,
                                            E0403,
                                            "the name `{}` is already used for a generic \
                                            parameter in this list of generic parameters",
                                            name);
             err.span_label(span, "already used");
             err.span_label(first_use_span, format!("first use of `{}`", name));
             err
        }
        ResolutionError::MethodNotMemberOfTrait(method, trait_) => {
            let mut err = struct_span_err!(resolver.session,
                                           span,
                                           E0407,
                                           "method `{}` is not a member of trait `{}`",
                                           method,
                                           trait_);
            err.span_label(span, format!("not a member of trait `{}`", trait_));
            err
        }
        ResolutionError::TypeNotMemberOfTrait(type_, trait_) => {
            let mut err = struct_span_err!(resolver.session,
                             span,
                             E0437,
                             "type `{}` is not a member of trait `{}`",
                             type_,
                             trait_);
            err.span_label(span, format!("not a member of trait `{}`", trait_));
            err
        }
        ResolutionError::ConstNotMemberOfTrait(const_, trait_) => {
            let mut err = struct_span_err!(resolver.session,
                             span,
                             E0438,
                             "const `{}` is not a member of trait `{}`",
                             const_,
                             trait_);
            err.span_label(span, format!("not a member of trait `{}`", trait_));
            err
        }
        ResolutionError::VariableNotBoundInPattern(binding_error) => {
            let target_sp = binding_error.target.iter().cloned().collect::<Vec<_>>();
            let msp = MultiSpan::from_spans(target_sp.clone());
            let msg = format!("variable `{}` is not bound in all patterns", binding_error.name);
            let mut err = resolver.session.struct_span_err_with_code(
                msp,
                &msg,
                DiagnosticId::Error("E0408".into()),
            );
            for sp in target_sp {
                err.span_label(sp, format!("pattern doesn't bind `{}`", binding_error.name));
            }
            let origin_sp = binding_error.origin.iter().cloned();
            for sp in origin_sp {
                err.span_label(sp, "variable not in all patterns");
            }
            err
        }
        ResolutionError::VariableBoundWithDifferentMode(variable_name,
                                                        first_binding_span) => {
            let mut err = struct_span_err!(resolver.session,
                             span,
                             E0409,
                             "variable `{}` is bound in inconsistent \
                             ways within the same match arm",
                             variable_name);
            err.span_label(span, "bound in different ways");
            err.span_label(first_binding_span, "first binding");
            err
        }
        ResolutionError::IdentifierBoundMoreThanOnceInParameterList(identifier) => {
            let mut err = struct_span_err!(resolver.session,
                             span,
                             E0415,
                             "identifier `{}` is bound more than once in this parameter list",
                             identifier);
            err.span_label(span, "used as parameter more than once");
            err
        }
        ResolutionError::IdentifierBoundMoreThanOnceInSamePattern(identifier) => {
            let mut err = struct_span_err!(resolver.session,
                             span,
                             E0416,
                             "identifier `{}` is bound more than once in the same pattern",
                             identifier);
            err.span_label(span, "used in a pattern more than once");
            err
        }
        ResolutionError::UndeclaredLabel(name, lev_candidate) => {
            let mut err = struct_span_err!(resolver.session,
                                           span,
                                           E0426,
                                           "use of undeclared label `{}`",
                                           name);
            if let Some(lev_candidate) = lev_candidate {
                err.span_suggestion(
                    span,
                    "a label with a similar name exists in this scope",
                    lev_candidate.to_string(),
                    Applicability::MaybeIncorrect,
                );
            } else {
                err.span_label(span, format!("undeclared label `{}`", name));
            }
            err
        }
        ResolutionError::SelfImportsOnlyAllowedWithin => {
            struct_span_err!(resolver.session,
                             span,
                             E0429,
                             "{}",
                             "`self` imports are only allowed within a { } list")
        }
        ResolutionError::SelfImportCanOnlyAppearOnceInTheList => {
            let mut err = struct_span_err!(resolver.session, span, E0430,
                                           "`self` import can only appear once in an import list");
            err.span_label(span, "can only appear once in an import list");
            err
        }
        ResolutionError::SelfImportOnlyInImportListWithNonEmptyPrefix => {
            let mut err = struct_span_err!(resolver.session, span, E0431,
                                           "`self` import can only appear in an import list with \
                                            a non-empty prefix");
            err.span_label(span, "can only appear in an import list with a non-empty prefix");
            err
        }
        ResolutionError::FailedToResolve { label, suggestion } => {
            let mut err = struct_span_err!(resolver.session, span, E0433,
                                           "failed to resolve: {}", &label);
            err.span_label(span, label);

            if let Some((suggestions, msg, applicability)) = suggestion {
                err.multipart_suggestion(&msg, suggestions, applicability);
            }

            err
        }
        ResolutionError::CannotCaptureDynamicEnvironmentInFnItem => {
            let mut err = struct_span_err!(resolver.session,
                                           span,
                                           E0434,
                                           "{}",
                                           "can't capture dynamic environment in a fn item");
            err.help("use the `|| { ... }` closure form instead");
            err
        }
        ResolutionError::AttemptToUseNonConstantValueInConstant => {
            let mut err = struct_span_err!(resolver.session, span, E0435,
                                           "attempt to use a non-constant value in a constant");
            err.span_label(span, "non-constant value");
            err
        }
        ResolutionError::BindingShadowsSomethingUnacceptable(what_binding, name, binding) => {
            let shadows_what = binding.descr();
            let mut err = struct_span_err!(resolver.session, span, E0530, "{}s cannot shadow {}s",
                                           what_binding, shadows_what);
            err.span_label(span, format!("cannot be named the same as {} {}",
                                         binding.article(), shadows_what));
            let participle = if binding.is_import() { "imported" } else { "defined" };
            let msg = format!("the {} `{}` is {} here", shadows_what, name, participle);
            err.span_label(binding.span, msg);
            err
        }
        ResolutionError::ForwardDeclaredTyParam => {
            let mut err = struct_span_err!(resolver.session, span, E0128,
                                           "type parameters with a default cannot use \
                                            forward declared identifiers");
            err.span_label(
                span, "defaulted type parameters cannot be forward declared".to_string());
            err
        }
        ResolutionError::ConstParamDependentOnTypeParam => {
            let mut err = struct_span_err!(
                resolver.session,
                span,
                E0671,
                "const parameters cannot depend on type parameters"
            );
            err.span_label(span, format!("const parameter depends on type parameter"));
            err
        }
    }
}

/// Adjust the impl span so that just the `impl` keyword is taken by removing
/// everything after `<` (`"impl<T> Iterator for A<T> {}" -> "impl"`) and
/// everything after the first whitespace (`"impl Iterator for A" -> "impl"`).
///
/// *Attention*: the method used is very fragile since it essentially duplicates the work of the
/// parser. If you need to use this function or something similar, please consider updating the
/// `source_map` functions and this function to something more robust.
fn reduce_impl_span_to_impl_keyword(cm: &SourceMap, impl_span: Span) -> Span {
    let impl_span = cm.span_until_char(impl_span, '<');
    let impl_span = cm.span_until_whitespace(impl_span);
    impl_span
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum AliasPossibility {
    No,
    Maybe,
}

#[derive(Copy, Clone, Debug)]
enum PathSource<'a> {
    // Type paths `Path`.
    Type,
    // Trait paths in bounds or impls.
    Trait(AliasPossibility),
    // Expression paths `path`, with optional parent context.
    Expr(Option<&'a Expr>),
    // Paths in path patterns `Path`.
    Pat,
    // Paths in struct expressions and patterns `Path { .. }`.
    Struct,
    // Paths in tuple struct patterns `Path(..)`.
    TupleStruct,
    // `m::A::B` in `<T as m::A>::B::C`.
    TraitItem(Namespace),
}

impl<'a> PathSource<'a> {
    fn namespace(self) -> Namespace {
        match self {
            PathSource::Type | PathSource::Trait(_) | PathSource::Struct => TypeNS,
            PathSource::Expr(..) | PathSource::Pat | PathSource::TupleStruct => ValueNS,
            PathSource::TraitItem(ns) => ns,
        }
    }

    fn defer_to_typeck(self) -> bool {
        match self {
            PathSource::Type | PathSource::Expr(..) | PathSource::Pat |
            PathSource::Struct | PathSource::TupleStruct => true,
            PathSource::Trait(_) | PathSource::TraitItem(..) => false,
        }
    }

    fn descr_expected(self) -> &'static str {
        match self {
            PathSource::Type => "type",
            PathSource::Trait(_) => "trait",
            PathSource::Pat => "unit struct/variant or constant",
            PathSource::Struct => "struct, variant or union type",
            PathSource::TupleStruct => "tuple struct/variant",
            PathSource::TraitItem(ns) => match ns {
                TypeNS => "associated type",
                ValueNS => "method or associated constant",
                MacroNS => bug!("associated macro"),
            },
            PathSource::Expr(parent) => match parent.map(|p| &p.node) {
                // "function" here means "anything callable" rather than `DefKind::Fn`,
                // this is not precise but usually more helpful than just "value".
                Some(&ExprKind::Call(..)) => "function",
                _ => "value",
            },
        }
    }

    fn is_expected(self, res: Res) -> bool {
        match self {
            PathSource::Type => match res {
                Res::Def(DefKind::Struct, _)
                | Res::Def(DefKind::Union, _)
                | Res::Def(DefKind::Enum, _)
                | Res::Def(DefKind::Trait, _)
                | Res::Def(DefKind::TraitAlias, _)
                | Res::Def(DefKind::TyAlias, _)
                | Res::Def(DefKind::AssocTy, _)
                | Res::PrimTy(..)
                | Res::Def(DefKind::TyParam, _)
                | Res::SelfTy(..)
                | Res::Def(DefKind::OpaqueTy, _)
                | Res::Def(DefKind::ForeignTy, _) => true,
                _ => false,
            },
            PathSource::Trait(AliasPossibility::No) => match res {
                Res::Def(DefKind::Trait, _) => true,
                _ => false,
            },
            PathSource::Trait(AliasPossibility::Maybe) => match res {
                Res::Def(DefKind::Trait, _) => true,
                Res::Def(DefKind::TraitAlias, _) => true,
                _ => false,
            },
            PathSource::Expr(..) => match res {
                Res::Def(DefKind::Ctor(_, CtorKind::Const), _)
                | Res::Def(DefKind::Ctor(_, CtorKind::Fn), _)
                | Res::Def(DefKind::Const, _)
                | Res::Def(DefKind::Static, _)
                | Res::Local(..)
                | Res::Def(DefKind::Fn, _)
                | Res::Def(DefKind::Method, _)
                | Res::Def(DefKind::AssocConst, _)
                | Res::SelfCtor(..)
                | Res::Def(DefKind::ConstParam, _) => true,
                _ => false,
            },
            PathSource::Pat => match res {
                Res::Def(DefKind::Ctor(_, CtorKind::Const), _) |
                Res::Def(DefKind::Const, _) | Res::Def(DefKind::AssocConst, _) |
                Res::SelfCtor(..) => true,
                _ => false,
            },
            PathSource::TupleStruct => match res {
                Res::Def(DefKind::Ctor(_, CtorKind::Fn), _) | Res::SelfCtor(..) => true,
                _ => false,
            },
            PathSource::Struct => match res {
                Res::Def(DefKind::Struct, _)
                | Res::Def(DefKind::Union, _)
                | Res::Def(DefKind::Variant, _)
                | Res::Def(DefKind::TyAlias, _)
                | Res::Def(DefKind::AssocTy, _)
                | Res::SelfTy(..) => true,
                _ => false,
            },
            PathSource::TraitItem(ns) => match res {
                Res::Def(DefKind::AssocConst, _)
                | Res::Def(DefKind::Method, _) if ns == ValueNS => true,
                Res::Def(DefKind::AssocTy, _) if ns == TypeNS => true,
                _ => false,
            },
        }
    }

    fn error_code(self, has_unexpected_resolution: bool) -> &'static str {
        __diagnostic_used!(E0404);
        __diagnostic_used!(E0405);
        __diagnostic_used!(E0412);
        __diagnostic_used!(E0422);
        __diagnostic_used!(E0423);
        __diagnostic_used!(E0425);
        __diagnostic_used!(E0531);
        __diagnostic_used!(E0532);
        __diagnostic_used!(E0573);
        __diagnostic_used!(E0574);
        __diagnostic_used!(E0575);
        __diagnostic_used!(E0576);
        match (self, has_unexpected_resolution) {
            (PathSource::Trait(_), true) => "E0404",
            (PathSource::Trait(_), false) => "E0405",
            (PathSource::Type, true) => "E0573",
            (PathSource::Type, false) => "E0412",
            (PathSource::Struct, true) => "E0574",
            (PathSource::Struct, false) => "E0422",
            (PathSource::Expr(..), true) => "E0423",
            (PathSource::Expr(..), false) => "E0425",
            (PathSource::Pat, true) | (PathSource::TupleStruct, true) => "E0532",
            (PathSource::Pat, false) | (PathSource::TupleStruct, false) => "E0531",
            (PathSource::TraitItem(..), true) => "E0575",
            (PathSource::TraitItem(..), false) => "E0576",
        }
    }
}

// A minimal representation of a path segment. We use this in resolve because
// we synthesize 'path segments' which don't have the rest of an AST or HIR
// `PathSegment`.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    ident: Ident,
    id: Option<NodeId>,
}

impl Segment {
    fn from_path(path: &Path) -> Vec<Segment> {
        path.segments.iter().map(|s| s.into()).collect()
    }

    fn from_ident(ident: Ident) -> Segment {
        Segment {
            ident,
            id: None,
        }
    }

    fn names_to_string(segments: &[Segment]) -> String {
        names_to_string(&segments.iter()
                            .map(|seg| seg.ident)
                            .collect::<Vec<_>>())
    }
}

impl<'a> From<&'a ast::PathSegment> for Segment {
    fn from(seg: &'a ast::PathSegment) -> Segment {
        Segment {
            ident: seg.ident,
            id: Some(seg.id),
        }
    }
}

struct UsePlacementFinder {
    target_module: NodeId,
    span: Option<Span>,
    found_use: bool,
}

impl UsePlacementFinder {
    fn check(krate: &Crate, target_module: NodeId) -> (Option<Span>, bool) {
        let mut finder = UsePlacementFinder {
            target_module,
            span: None,
            found_use: false,
        };
        visit::walk_crate(&mut finder, krate);
        (finder.span, finder.found_use)
    }
}

impl<'tcx> Visitor<'tcx> for UsePlacementFinder {
    fn visit_mod(
        &mut self,
        module: &'tcx ast::Mod,
        _: Span,
        _: &[ast::Attribute],
        node_id: NodeId,
    ) {
        if self.span.is_some() {
            return;
        }
        if node_id != self.target_module {
            visit::walk_mod(self, module);
            return;
        }
        // find a use statement
        for item in &module.items {
            match item.node {
                ItemKind::Use(..) => {
                    // don't suggest placing a use before the prelude
                    // import or other generated ones
                    if item.span.ctxt().outer_expn_info().is_none() {
                        self.span = Some(item.span.shrink_to_lo());
                        self.found_use = true;
                        return;
                    }
                },
                // don't place use before extern crate
                ItemKind::ExternCrate(_) => {}
                // but place them before the first other item
                _ => if self.span.map_or(true, |span| item.span < span ) {
                    if item.span.ctxt().outer_expn_info().is_none() {
                        // don't insert between attributes and an item
                        if item.attrs.is_empty() {
                            self.span = Some(item.span.shrink_to_lo());
                        } else {
                            // find the first attribute on the item
                            for attr in &item.attrs {
                                if self.span.map_or(true, |span| attr.span < span) {
                                    self.span = Some(attr.span.shrink_to_lo());
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

/// The rib kind restricts certain accesses,
/// e.g. to a `Res::Local` of an outer item.
#[derive(Copy, Clone, Debug)]
enum RibKind<'a> {
    /// No restriction needs to be applied.
    NormalRibKind,

    /// We passed through an impl or trait and are now in one of its
    /// methods or associated types. Allow references to ty params that impl or trait
    /// binds. Disallow any other upvars (including other ty params that are
    /// upvars).
    AssocItemRibKind,

    /// We passed through a function definition. Disallow upvars.
    /// Permit only those const parameters that are specified in the function's generics.
    FnItemRibKind,

    /// We passed through an item scope. Disallow upvars.
    ItemRibKind,

    /// We're in a constant item. Can't refer to dynamic stuff.
    ConstantItemRibKind,

    /// We passed through a module.
    ModuleRibKind(Module<'a>),

    /// We passed through a `macro_rules!` statement
    MacroDefinition(DefId),

    /// All bindings in this rib are type parameters that can't be used
    /// from the default of a type parameter because they're not declared
    /// before said type parameter. Also see the `visit_generics` override.
    ForwardTyParamBanRibKind,

    /// We forbid the use of type parameters as the types of const parameters.
    TyParamAsConstParamTy,
}

/// A single local scope.
///
/// A rib represents a scope names can live in. Note that these appear in many places, not just
/// around braces. At any place where the list of accessible names (of the given namespace)
/// changes or a new restrictions on the name accessibility are introduced, a new rib is put onto a
/// stack. This may be, for example, a `let` statement (because it introduces variables), a macro,
/// etc.
///
/// Different [rib kinds](enum.RibKind) are transparent for different names.
///
/// The resolution keeps a separate stack of ribs as it traverses the AST for each namespace. When
/// resolving, the name is looked up from inside out.
#[derive(Debug)]
struct Rib<'a, R = Res> {
    bindings: FxHashMap<Ident, R>,
    kind: RibKind<'a>,
}

impl<'a, R> Rib<'a, R> {
    fn new(kind: RibKind<'a>) -> Rib<'a, R> {
        Rib {
            bindings: Default::default(),
            kind,
        }
    }
}

/// An intermediate resolution result.
///
/// This refers to the thing referred by a name. The difference between `Res` and `Item` is that
/// items are visible in their whole block, while `Res`es only from the place they are defined
/// forward.
#[derive(Debug)]
enum LexicalScopeBinding<'a> {
    Item(&'a NameBinding<'a>),
    Res(Res),
}

impl<'a> LexicalScopeBinding<'a> {
    fn item(self) -> Option<&'a NameBinding<'a>> {
        match self {
            LexicalScopeBinding::Item(binding) => Some(binding),
            _ => None,
        }
    }

    fn res(self) -> Res {
        match self {
            LexicalScopeBinding::Item(binding) => binding.res(),
            LexicalScopeBinding::Res(res) => res,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ModuleOrUniformRoot<'a> {
    /// Regular module.
    Module(Module<'a>),

    /// Virtual module that denotes resolution in crate root with fallback to extern prelude.
    CrateRootAndExternPrelude,

    /// Virtual module that denotes resolution in extern prelude.
    /// Used for paths starting with `::` on 2018 edition.
    ExternPrelude,

    /// Virtual module that denotes resolution in current scope.
    /// Used only for resolving single-segment imports. The reason it exists is that import paths
    /// are always split into two parts, the first of which should be some kind of module.
    CurrentScope,
}

impl ModuleOrUniformRoot<'_> {
    fn same_def(lhs: Self, rhs: Self) -> bool {
        match (lhs, rhs) {
            (ModuleOrUniformRoot::Module(lhs),
             ModuleOrUniformRoot::Module(rhs)) => lhs.def_id() == rhs.def_id(),
            (ModuleOrUniformRoot::CrateRootAndExternPrelude,
             ModuleOrUniformRoot::CrateRootAndExternPrelude) |
            (ModuleOrUniformRoot::ExternPrelude, ModuleOrUniformRoot::ExternPrelude) |
            (ModuleOrUniformRoot::CurrentScope, ModuleOrUniformRoot::CurrentScope) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
enum PathResult<'a> {
    Module(ModuleOrUniformRoot<'a>),
    NonModule(PartialRes),
    Indeterminate,
    Failed {
        span: Span,
        label: String,
        suggestion: Option<Suggestion>,
        is_error_from_last_segment: bool,
    },
}

enum ModuleKind {
    /// An anonymous module; e.g., just a block.
    ///
    /// ```
    /// fn main() {
    ///     fn f() {} // (1)
    ///     { // This is an anonymous module
    ///         f(); // This resolves to (2) as we are inside the block.
    ///         fn f() {} // (2)
    ///     }
    ///     f(); // Resolves to (1)
    /// }
    /// ```
    Block(NodeId),
    /// Any module with a name.
    ///
    /// This could be:
    ///
    /// * A normal module ‒ either `mod from_file;` or `mod from_block { }`.
    /// * A trait or an enum (it implicitly contains associated types, methods and variant
    ///   constructors).
    Def(DefKind, DefId, Name),
}

impl ModuleKind {
    /// Get name of the module.
    pub fn name(&self) -> Option<Name> {
        match self {
            ModuleKind::Block(..) => None,
            ModuleKind::Def(.., name) => Some(*name),
        }
    }
}

/// One node in the tree of modules.
pub struct ModuleData<'a> {
    parent: Option<Module<'a>>,
    kind: ModuleKind,

    // The def id of the closest normal module (`mod`) ancestor (including this module).
    normal_ancestor_id: DefId,

    resolutions: RefCell<FxHashMap<(Ident, Namespace), &'a RefCell<NameResolution<'a>>>>,
    single_segment_macro_resolutions: RefCell<Vec<(Ident, MacroKind, ParentScope<'a>,
                                                   Option<&'a NameBinding<'a>>)>>,
    multi_segment_macro_resolutions: RefCell<Vec<(Vec<Segment>, Span, MacroKind, ParentScope<'a>,
                                                  Option<Res>)>>,
    builtin_attrs: RefCell<Vec<(Ident, ParentScope<'a>)>>,

    // Macro invocations that can expand into items in this module.
    unresolved_invocations: RefCell<FxHashSet<ExpnId>>,

    no_implicit_prelude: bool,

    glob_importers: RefCell<Vec<&'a ImportDirective<'a>>>,
    globs: RefCell<Vec<&'a ImportDirective<'a>>>,

    // Used to memoize the traits in this module for faster searches through all traits in scope.
    traits: RefCell<Option<Box<[(Ident, &'a NameBinding<'a>)]>>>,

    // Whether this module is populated. If not populated, any attempt to
    // access the children must be preceded with a
    // `populate_module_if_necessary` call.
    populated: Cell<bool>,

    /// Span of the module itself. Used for error reporting.
    span: Span,

    expansion: ExpnId,
}

type Module<'a> = &'a ModuleData<'a>;

impl<'a> ModuleData<'a> {
    fn new(parent: Option<Module<'a>>,
           kind: ModuleKind,
           normal_ancestor_id: DefId,
           expansion: ExpnId,
           span: Span) -> Self {
        ModuleData {
            parent,
            kind,
            normal_ancestor_id,
            resolutions: Default::default(),
            single_segment_macro_resolutions: RefCell::new(Vec::new()),
            multi_segment_macro_resolutions: RefCell::new(Vec::new()),
            builtin_attrs: RefCell::new(Vec::new()),
            unresolved_invocations: Default::default(),
            no_implicit_prelude: false,
            glob_importers: RefCell::new(Vec::new()),
            globs: RefCell::new(Vec::new()),
            traits: RefCell::new(None),
            populated: Cell::new(normal_ancestor_id.is_local()),
            span,
            expansion,
        }
    }

    fn for_each_child<F: FnMut(Ident, Namespace, &'a NameBinding<'a>)>(&self, mut f: F) {
        for (&(ident, ns), name_resolution) in self.resolutions.borrow().iter() {
            name_resolution.borrow().binding.map(|binding| f(ident, ns, binding));
        }
    }

    fn for_each_child_stable<F: FnMut(Ident, Namespace, &'a NameBinding<'a>)>(&self, mut f: F) {
        let resolutions = self.resolutions.borrow();
        let mut resolutions = resolutions.iter().collect::<Vec<_>>();
        resolutions.sort_by_cached_key(|&(&(ident, ns), _)| (ident.as_str(), ns));
        for &(&(ident, ns), &resolution) in resolutions.iter() {
            resolution.borrow().binding.map(|binding| f(ident, ns, binding));
        }
    }

    fn res(&self) -> Option<Res> {
        match self.kind {
            ModuleKind::Def(kind, def_id, _) => Some(Res::Def(kind, def_id)),
            _ => None,
        }
    }

    fn def_kind(&self) -> Option<DefKind> {
        match self.kind {
            ModuleKind::Def(kind, ..) => Some(kind),
            _ => None,
        }
    }

    fn def_id(&self) -> Option<DefId> {
        match self.kind {
            ModuleKind::Def(_, def_id, _) => Some(def_id),
            _ => None,
        }
    }

    // `self` resolves to the first module ancestor that `is_normal`.
    fn is_normal(&self) -> bool {
        match self.kind {
            ModuleKind::Def(DefKind::Mod, _, _) => true,
            _ => false,
        }
    }

    fn is_trait(&self) -> bool {
        match self.kind {
            ModuleKind::Def(DefKind::Trait, _, _) => true,
            _ => false,
        }
    }

    fn nearest_item_scope(&'a self) -> Module<'a> {
        if self.is_trait() { self.parent.unwrap() } else { self }
    }

    fn is_ancestor_of(&self, mut other: &Self) -> bool {
        while !ptr::eq(self, other) {
            if let Some(parent) = other.parent {
                other = parent;
            } else {
                return false;
            }
        }
        true
    }
}

impl<'a> fmt::Debug for ModuleData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.res())
    }
}

/// Records a possibly-private value, type, or module definition.
#[derive(Clone, Debug)]
pub struct NameBinding<'a> {
    kind: NameBindingKind<'a>,
    ambiguity: Option<(&'a NameBinding<'a>, AmbiguityKind)>,
    expansion: ExpnId,
    span: Span,
    vis: ty::Visibility,
}

pub trait ToNameBinding<'a> {
    fn to_name_binding(self, arenas: &'a ResolverArenas<'a>) -> &'a NameBinding<'a>;
}

impl<'a> ToNameBinding<'a> for &'a NameBinding<'a> {
    fn to_name_binding(self, _: &'a ResolverArenas<'a>) -> &'a NameBinding<'a> {
        self
    }
}

#[derive(Clone, Debug)]
enum NameBindingKind<'a> {
    Res(Res, /* is_macro_export */ bool),
    Module(Module<'a>),
    Import {
        binding: &'a NameBinding<'a>,
        directive: &'a ImportDirective<'a>,
        used: Cell<bool>,
    },
}

impl<'a> NameBindingKind<'a> {
    /// Is this a name binding of a import?
    fn is_import(&self) -> bool {
        match *self {
            NameBindingKind::Import { .. } => true,
            _ => false,
        }
    }
}

struct PrivacyError<'a>(Span, Ident, &'a NameBinding<'a>);

struct UseError<'a> {
    err: DiagnosticBuilder<'a>,
    /// Attach `use` statements for these candidates.
    candidates: Vec<ImportSuggestion>,
    /// The `NodeId` of the module to place the use-statements in.
    node_id: NodeId,
    /// Whether the diagnostic should state that it's "better".
    better: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum AmbiguityKind {
    Import,
    BuiltinAttr,
    DeriveHelper,
    LegacyHelperVsPrelude,
    LegacyVsModern,
    GlobVsOuter,
    GlobVsGlob,
    GlobVsExpanded,
    MoreExpandedVsOuter,
}

impl AmbiguityKind {
    fn descr(self) -> &'static str {
        match self {
            AmbiguityKind::Import =>
                "name vs any other name during import resolution",
            AmbiguityKind::BuiltinAttr =>
                "built-in attribute vs any other name",
            AmbiguityKind::DeriveHelper =>
                "derive helper attribute vs any other name",
            AmbiguityKind::LegacyHelperVsPrelude =>
                "legacy plugin helper attribute vs name from prelude",
            AmbiguityKind::LegacyVsModern =>
                "`macro_rules` vs non-`macro_rules` from other module",
            AmbiguityKind::GlobVsOuter =>
                "glob import vs any other name from outer scope during import/macro resolution",
            AmbiguityKind::GlobVsGlob =>
                "glob import vs glob import in the same module",
            AmbiguityKind::GlobVsExpanded =>
                "glob import vs macro-expanded name in the same \
                 module during import/macro resolution",
            AmbiguityKind::MoreExpandedVsOuter =>
                "macro-expanded name vs less macro-expanded name \
                 from outer scope during import/macro resolution",
        }
    }
}

/// Miscellaneous bits of metadata for better ambiguity error reporting.
#[derive(Clone, Copy, PartialEq)]
enum AmbiguityErrorMisc {
    SuggestCrate,
    SuggestSelf,
    FromPrelude,
    None,
}

struct AmbiguityError<'a> {
    kind: AmbiguityKind,
    ident: Ident,
    b1: &'a NameBinding<'a>,
    b2: &'a NameBinding<'a>,
    misc1: AmbiguityErrorMisc,
    misc2: AmbiguityErrorMisc,
}

impl<'a> NameBinding<'a> {
    fn module(&self) -> Option<Module<'a>> {
        match self.kind {
            NameBindingKind::Module(module) => Some(module),
            NameBindingKind::Import { binding, .. } => binding.module(),
            _ => None,
        }
    }

    fn res(&self) -> Res {
        match self.kind {
            NameBindingKind::Res(res, _) => res,
            NameBindingKind::Module(module) => module.res().unwrap(),
            NameBindingKind::Import { binding, .. } => binding.res(),
        }
    }

    fn is_ambiguity(&self) -> bool {
        self.ambiguity.is_some() || match self.kind {
            NameBindingKind::Import { binding, .. } => binding.is_ambiguity(),
            _ => false,
        }
    }

    // We sometimes need to treat variants as `pub` for backwards compatibility.
    fn pseudo_vis(&self) -> ty::Visibility {
        if self.is_variant() && self.res().def_id().is_local() {
            ty::Visibility::Public
        } else {
            self.vis
        }
    }

    fn is_variant(&self) -> bool {
        match self.kind {
            NameBindingKind::Res(Res::Def(DefKind::Variant, _), _) |
            NameBindingKind::Res(Res::Def(DefKind::Ctor(CtorOf::Variant, ..), _), _) => true,
            _ => false,
        }
    }

    fn is_extern_crate(&self) -> bool {
        match self.kind {
            NameBindingKind::Import {
                directive: &ImportDirective {
                    subclass: ImportDirectiveSubclass::ExternCrate { .. }, ..
                }, ..
            } => true,
            NameBindingKind::Module(
                &ModuleData { kind: ModuleKind::Def(DefKind::Mod, def_id, _), .. }
            ) => def_id.index == CRATE_DEF_INDEX,
            _ => false,
        }
    }

    fn is_import(&self) -> bool {
        match self.kind {
            NameBindingKind::Import { .. } => true,
            _ => false,
        }
    }

    fn is_glob_import(&self) -> bool {
        match self.kind {
            NameBindingKind::Import { directive, .. } => directive.is_glob(),
            _ => false,
        }
    }

    fn is_importable(&self) -> bool {
        match self.res() {
            Res::Def(DefKind::AssocConst, _)
            | Res::Def(DefKind::Method, _)
            | Res::Def(DefKind::AssocTy, _) => false,
            _ => true,
        }
    }

    fn is_macro_def(&self) -> bool {
        match self.kind {
            NameBindingKind::Res(Res::Def(DefKind::Macro(..), _), _) => true,
            _ => false,
        }
    }

    fn macro_kind(&self) -> Option<MacroKind> {
        self.res().macro_kind()
    }

    fn descr(&self) -> &'static str {
        if self.is_extern_crate() { "extern crate" } else { self.res().descr() }
    }

    fn article(&self) -> &'static str {
        if self.is_extern_crate() { "an" } else { self.res().article() }
    }

    // Suppose that we resolved macro invocation with `invoc_parent_expansion` to binding `binding`
    // at some expansion round `max(invoc, binding)` when they both emerged from macros.
    // Then this function returns `true` if `self` may emerge from a macro *after* that
    // in some later round and screw up our previously found resolution.
    // See more detailed explanation in
    // https://github.com/rust-lang/rust/pull/53778#issuecomment-419224049
    fn may_appear_after(&self, invoc_parent_expansion: ExpnId, binding: &NameBinding<'_>) -> bool {
        // self > max(invoc, binding) => !(self <= invoc || self <= binding)
        // Expansions are partially ordered, so "may appear after" is an inversion of
        // "certainly appears before or simultaneously" and includes unordered cases.
        let self_parent_expansion = self.expansion;
        let other_parent_expansion = binding.expansion;
        let certainly_before_other_or_simultaneously =
            other_parent_expansion.is_descendant_of(self_parent_expansion);
        let certainly_before_invoc_or_simultaneously =
            invoc_parent_expansion.is_descendant_of(self_parent_expansion);
        !(certainly_before_other_or_simultaneously || certainly_before_invoc_or_simultaneously)
    }
}

/// Interns the names of the primitive types.
///
/// All other types are defined somewhere and possibly imported, but the primitive ones need
/// special handling, since they have no place of origin.
struct PrimitiveTypeTable {
    primitive_types: FxHashMap<Name, PrimTy>,
}

impl PrimitiveTypeTable {
    fn new() -> PrimitiveTypeTable {
        let mut table = FxHashMap::default();

        table.insert(sym::bool, Bool);
        table.insert(sym::char, Char);
        table.insert(sym::f32, Float(FloatTy::F32));
        table.insert(sym::f64, Float(FloatTy::F64));
        table.insert(sym::isize, Int(IntTy::Isize));
        table.insert(sym::i8, Int(IntTy::I8));
        table.insert(sym::i16, Int(IntTy::I16));
        table.insert(sym::i32, Int(IntTy::I32));
        table.insert(sym::i64, Int(IntTy::I64));
        table.insert(sym::i128, Int(IntTy::I128));
        table.insert(sym::str, Str);
        table.insert(sym::usize, Uint(UintTy::Usize));
        table.insert(sym::u8, Uint(UintTy::U8));
        table.insert(sym::u16, Uint(UintTy::U16));
        table.insert(sym::u32, Uint(UintTy::U32));
        table.insert(sym::u64, Uint(UintTy::U64));
        table.insert(sym::u128, Uint(UintTy::U128));
        Self { primitive_types: table }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExternPreludeEntry<'a> {
    extern_crate_item: Option<&'a NameBinding<'a>>,
    pub introduced_by_item: bool,
}

/// The main resolver class.
///
/// This is the visitor that walks the whole crate.
pub struct Resolver<'a> {
    session: &'a Session,
    cstore: &'a CStore,

    pub definitions: Definitions,

    graph_root: Module<'a>,

    prelude: Option<Module<'a>>,
    pub extern_prelude: FxHashMap<Ident, ExternPreludeEntry<'a>>,

    /// N.B., this is used only for better diagnostics, not name resolution itself.
    has_self: FxHashSet<DefId>,

    /// Names of fields of an item `DefId` accessible with dot syntax.
    /// Used for hints during error reporting.
    field_names: FxHashMap<DefId, Vec<Name>>,

    /// All imports known to succeed or fail.
    determined_imports: Vec<&'a ImportDirective<'a>>,

    /// All non-determined imports.
    indeterminate_imports: Vec<&'a ImportDirective<'a>>,

    /// FIXME: Refactor things so that these fields are passed through arguments and not resolver.
    /// We are resolving a last import segment during import validation.
    last_import_segment: bool,
    /// This binding should be ignored during in-module resolution, so that we don't get
    /// "self-confirming" import resolutions during import validation.
    blacklisted_binding: Option<&'a NameBinding<'a>>,

    /// The idents for the primitive types.
    primitive_type_table: PrimitiveTypeTable,

    /// Resolutions for nodes that have a single resolution.
    partial_res_map: NodeMap<PartialRes>,
    /// Resolutions for import nodes, which have multiple resolutions in different namespaces.
    import_res_map: NodeMap<PerNS<Option<Res>>>,
    /// Resolutions for labels (node IDs of their corresponding blocks or loops).
    label_res_map: NodeMap<NodeId>,

    pub export_map: ExportMap<NodeId>,
    pub trait_map: TraitMap,

    /// A map from nodes to anonymous modules.
    /// Anonymous modules are pseudo-modules that are implicitly created around items
    /// contained within blocks.
    ///
    /// For example, if we have this:
    ///
    ///  fn f() {
    ///      fn g() {
    ///          ...
    ///      }
    ///  }
    ///
    /// There will be an anonymous module created around `g` with the ID of the
    /// entry block for `f`.
    block_map: NodeMap<Module<'a>>,
    module_map: FxHashMap<DefId, Module<'a>>,
    extern_module_map: FxHashMap<(DefId, bool /* MacrosOnly? */), Module<'a>>,
    binding_parent_modules: FxHashMap<PtrKey<'a, NameBinding<'a>>, Module<'a>>,

    /// Maps glob imports to the names of items actually imported.
    pub glob_map: GlobMap,

    used_imports: FxHashSet<(NodeId, Namespace)>,
    pub maybe_unused_trait_imports: NodeSet,
    pub maybe_unused_extern_crates: Vec<(NodeId, Span)>,

    /// Privacy errors are delayed until the end in order to deduplicate them.
    privacy_errors: Vec<PrivacyError<'a>>,
    /// Ambiguity errors are delayed for deduplication.
    ambiguity_errors: Vec<AmbiguityError<'a>>,
    /// `use` injections are delayed for better placement and deduplication.
    use_injections: Vec<UseError<'a>>,
    /// Crate-local macro expanded `macro_export` referred to by a module-relative path.
    macro_expanded_macro_export_errors: BTreeSet<(Span, Span)>,

    arenas: &'a ResolverArenas<'a>,
    dummy_binding: &'a NameBinding<'a>,

    crate_loader: &'a mut CrateLoader<'a>,
    macro_names: FxHashSet<Ident>,
    builtin_macros: FxHashMap<Name, SyntaxExtension>,
    macro_use_prelude: FxHashMap<Name, &'a NameBinding<'a>>,
    pub all_macros: FxHashMap<Name, Res>,
    macro_map: FxHashMap<DefId, Lrc<SyntaxExtension>>,
    dummy_ext_bang: Lrc<SyntaxExtension>,
    dummy_ext_derive: Lrc<SyntaxExtension>,
    non_macro_attrs: [Lrc<SyntaxExtension>; 2],
    macro_defs: FxHashMap<ExpnId, DefId>,
    local_macro_def_scopes: FxHashMap<NodeId, Module<'a>>,
    unused_macros: NodeMap<Span>,
    proc_macro_stubs: NodeSet,
    /// Some built-in derives mark items they are applied to so they are treated specially later.
    /// Derive macros cannot modify the item themselves and have to store the markers in the global
    /// context, so they attach the markers to derive container IDs using this resolver table.
    /// FIXME: Find a way for `PartialEq` and `Eq` to emulate `#[structural_match]`
    /// by marking the produced impls rather than the original items.
    special_derives: FxHashMap<ExpnId, SpecialDerives>,

    /// Maps the `ExpnId` of an expansion to its containing module or block.
    invocations: FxHashMap<ExpnId, &'a InvocationData<'a>>,

    /// Avoid duplicated errors for "name already defined".
    name_already_seen: FxHashMap<Name, Span>,

    potentially_unused_imports: Vec<&'a ImportDirective<'a>>,

    /// Table for mapping struct IDs into struct constructor IDs,
    /// it's not used during normal resolution, only for better error reporting.
    struct_constructors: DefIdMap<(Res, ty::Visibility)>,

    injected_crate: Option<Module<'a>>,

    /// Features enabled for this crate.
    active_features: FxHashSet<Symbol>,
}

/// Nothing really interesting here; it just provides memory for the rest of the crate.
#[derive(Default)]
pub struct ResolverArenas<'a> {
    modules: arena::TypedArena<ModuleData<'a>>,
    local_modules: RefCell<Vec<Module<'a>>>,
    name_bindings: arena::TypedArena<NameBinding<'a>>,
    import_directives: arena::TypedArena<ImportDirective<'a>>,
    name_resolutions: arena::TypedArena<RefCell<NameResolution<'a>>>,
    invocation_data: arena::TypedArena<InvocationData<'a>>,
    legacy_bindings: arena::TypedArena<LegacyBinding<'a>>,
}

impl<'a> ResolverArenas<'a> {
    fn alloc_module(&'a self, module: ModuleData<'a>) -> Module<'a> {
        let module = self.modules.alloc(module);
        if module.def_id().map(|def_id| def_id.is_local()).unwrap_or(true) {
            self.local_modules.borrow_mut().push(module);
        }
        module
    }
    fn local_modules(&'a self) -> std::cell::Ref<'a, Vec<Module<'a>>> {
        self.local_modules.borrow()
    }
    fn alloc_name_binding(&'a self, name_binding: NameBinding<'a>) -> &'a NameBinding<'a> {
        self.name_bindings.alloc(name_binding)
    }
    fn alloc_import_directive(&'a self, import_directive: ImportDirective<'a>)
                              -> &'a ImportDirective<'_> {
        self.import_directives.alloc(import_directive)
    }
    fn alloc_name_resolution(&'a self) -> &'a RefCell<NameResolution<'a>> {
        self.name_resolutions.alloc(Default::default())
    }
    fn alloc_invocation_data(&'a self, expansion_data: InvocationData<'a>)
                             -> &'a InvocationData<'a> {
        self.invocation_data.alloc(expansion_data)
    }
    fn alloc_legacy_binding(&'a self, binding: LegacyBinding<'a>) -> &'a LegacyBinding<'a> {
        self.legacy_bindings.alloc(binding)
    }
}

impl<'a, 'b> ty::DefIdTree for &'a Resolver<'b> {
    fn parent(self, id: DefId) -> Option<DefId> {
        match id.krate {
            LOCAL_CRATE => self.definitions.def_key(id.index).parent,
            _ => self.cstore.def_key(id).parent,
        }.map(|index| DefId { index, ..id })
    }
}

/// This interface is used through the AST→HIR step, to embed full paths into the HIR. After that
/// the resolver is no longer needed as all the relevant information is inline.
impl<'a> hir::lowering::Resolver for Resolver<'a> {
    fn resolve_ast_path(
        &mut self,
        path: &ast::Path,
        is_value: bool,
    ) -> Res {
        match self.resolve_ast_path_inner(path, is_value) {
            Ok(r) => r,
            Err((span, error)) => {
                resolve_error(self, span, error);
                Res::Err
            }
        }
    }

    fn resolve_str_path(
        &mut self,
        span: Span,
        crate_root: Option<Symbol>,
        components: &[Symbol],
        is_value: bool
    ) -> (ast::Path, Res) {
        let root = if crate_root.is_some() {
            kw::PathRoot
        } else {
            kw::Crate
        };
        let segments = iter::once(Ident::with_empty_ctxt(root))
            .chain(
                crate_root.into_iter()
                    .chain(components.iter().cloned())
                    .map(Ident::with_empty_ctxt)
            ).map(|i| self.new_ast_path_segment(i)).collect::<Vec<_>>();

        let path = ast::Path {
            span,
            segments,
        };

        let res = self.resolve_ast_path(&path, is_value);
        (path, res)
    }

    fn get_partial_res(&mut self, id: NodeId) -> Option<PartialRes> {
        self.partial_res_map.get(&id).cloned()
    }

    fn get_import_res(&mut self, id: NodeId) -> PerNS<Option<Res>> {
        self.import_res_map.get(&id).cloned().unwrap_or_default()
    }

    fn get_label_res(&mut self, id: NodeId) -> Option<NodeId> {
        self.label_res_map.get(&id).cloned()
    }

    fn definitions(&mut self) -> &mut Definitions {
        &mut self.definitions
    }

    fn has_derives(&self, node_id: NodeId, derives: SpecialDerives) -> bool {
        let def_id = self.definitions.local_def_id(node_id);
        let expn_id = self.definitions.expansion_that_defined(def_id.index);
        self.has_derives(expn_id, derives)
    }
}

impl<'a> Resolver<'a> {
    pub fn new(session: &'a Session,
               cstore: &'a CStore,
               krate: &Crate,
               crate_name: &str,
               crate_loader: &'a mut CrateLoader<'a>,
               arenas: &'a ResolverArenas<'a>)
               -> Resolver<'a> {
        let root_def_id = DefId::local(CRATE_DEF_INDEX);
        let root_module_kind = ModuleKind::Def(
            DefKind::Mod,
            root_def_id,
            kw::Invalid,
        );
        let graph_root = arenas.alloc_module(ModuleData {
            no_implicit_prelude: attr::contains_name(&krate.attrs, sym::no_implicit_prelude),
            ..ModuleData::new(None, root_module_kind, root_def_id, ExpnId::root(), krate.span)
        });
        let mut module_map = FxHashMap::default();
        module_map.insert(DefId::local(CRATE_DEF_INDEX), graph_root);

        let mut definitions = Definitions::default();
        definitions.create_root_def(crate_name, session.local_crate_disambiguator());

        let mut extern_prelude: FxHashMap<Ident, ExternPreludeEntry<'_>> =
            session.opts.externs.iter().map(|kv| (Ident::from_str(kv.0), Default::default()))
                                       .collect();

        if !attr::contains_name(&krate.attrs, sym::no_core) {
            extern_prelude.insert(Ident::with_empty_ctxt(sym::core), Default::default());
            if !attr::contains_name(&krate.attrs, sym::no_std) {
                extern_prelude.insert(Ident::with_empty_ctxt(sym::std), Default::default());
                if session.rust_2018() {
                    extern_prelude.insert(Ident::with_empty_ctxt(sym::meta), Default::default());
                }
            }
        }

        let mut invocations = FxHashMap::default();
        invocations.insert(ExpnId::root(),
                           arenas.alloc_invocation_data(InvocationData::root(graph_root)));

        let mut macro_defs = FxHashMap::default();
        macro_defs.insert(ExpnId::root(), root_def_id);

        let features = session.features_untracked();
        let non_macro_attr =
            |mark_used| Lrc::new(SyntaxExtension::non_macro_attr(mark_used, session.edition()));

        Resolver {
            session,

            cstore,

            definitions,

            // The outermost module has def ID 0; this is not reflected in the
            // AST.
            graph_root,
            prelude: None,
            extern_prelude,

            has_self: FxHashSet::default(),
            field_names: FxHashMap::default(),

            determined_imports: Vec::new(),
            indeterminate_imports: Vec::new(),

            last_import_segment: false,
            blacklisted_binding: None,

            primitive_type_table: PrimitiveTypeTable::new(),

            partial_res_map: Default::default(),
            import_res_map: Default::default(),
            label_res_map: Default::default(),
            export_map: FxHashMap::default(),
            trait_map: Default::default(),
            module_map,
            block_map: Default::default(),
            extern_module_map: FxHashMap::default(),
            binding_parent_modules: FxHashMap::default(),

            glob_map: Default::default(),

            used_imports: FxHashSet::default(),
            maybe_unused_trait_imports: Default::default(),
            maybe_unused_extern_crates: Vec::new(),

            privacy_errors: Vec::new(),
            ambiguity_errors: Vec::new(),
            use_injections: Vec::new(),
            macro_expanded_macro_export_errors: BTreeSet::new(),

            arenas,
            dummy_binding: arenas.alloc_name_binding(NameBinding {
                kind: NameBindingKind::Res(Res::Err, false),
                ambiguity: None,
                expansion: ExpnId::root(),
                span: DUMMY_SP,
                vis: ty::Visibility::Public,
            }),

            crate_loader,
            macro_names: FxHashSet::default(),
            builtin_macros: Default::default(),
            macro_use_prelude: FxHashMap::default(),
            all_macros: FxHashMap::default(),
            macro_map: FxHashMap::default(),
            dummy_ext_bang: Lrc::new(SyntaxExtension::dummy_bang(session.edition())),
            dummy_ext_derive: Lrc::new(SyntaxExtension::dummy_derive(session.edition())),
            non_macro_attrs: [non_macro_attr(false), non_macro_attr(true)],
            invocations,
            macro_defs,
            local_macro_def_scopes: FxHashMap::default(),
            name_already_seen: FxHashMap::default(),
            potentially_unused_imports: Vec::new(),
            struct_constructors: Default::default(),
            unused_macros: Default::default(),
            proc_macro_stubs: Default::default(),
            special_derives: Default::default(),
            injected_crate: None,
            active_features:
                features.declared_lib_features.iter().map(|(feat, ..)| *feat)
                    .chain(features.declared_lang_features.iter().map(|(feat, ..)| *feat))
                    .collect(),
        }
    }

    pub fn arenas() -> ResolverArenas<'a> {
        Default::default()
    }

    fn non_macro_attr(&self, mark_used: bool) -> Lrc<SyntaxExtension> {
        self.non_macro_attrs[mark_used as usize].clone()
    }

    fn dummy_ext(&self, macro_kind: MacroKind) -> Lrc<SyntaxExtension> {
        match macro_kind {
            MacroKind::Bang => self.dummy_ext_bang.clone(),
            MacroKind::Derive => self.dummy_ext_derive.clone(),
            MacroKind::Attr => self.non_macro_attr(true),
        }
    }

    /// Runs the function on each namespace.
    fn per_ns<F: FnMut(&mut Self, Namespace)>(&mut self, mut f: F) {
        f(self, TypeNS);
        f(self, ValueNS);
        f(self, MacroNS);
    }

    fn is_builtin_macro(&mut self, def_id: Option<DefId>) -> bool {
        def_id.and_then(|def_id| self.get_macro_by_def_id(def_id))
              .map_or(false, |ext| ext.is_builtin)
    }

    fn macro_def(&self, mut ctxt: SyntaxContext) -> DefId {
        loop {
            match self.macro_defs.get(&ctxt.outer_expn()) {
                Some(&def_id) => return def_id,
                None => ctxt.remove_mark(),
            };
        }
    }

    fn has_derives(&self, expn_id: ExpnId, markers: SpecialDerives) -> bool {
        self.special_derives.get(&expn_id).map_or(false, |m| m.contains(markers))
    }

    /// Entry point to crate resolution.
    pub fn resolve_crate(&mut self, krate: &Crate) {
        ImportResolver { r: self }.finalize_imports();

        self.late_resolve_crate(krate);

        check_unused::check_crate(self, krate);
        self.report_errors(krate);
        self.crate_loader.postprocess(krate);
    }

    fn new_module(
        &self,
        parent: Module<'a>,
        kind: ModuleKind,
        normal_ancestor_id: DefId,
        expn_id: ExpnId,
        span: Span,
    ) -> Module<'a> {
        let module = ModuleData::new(Some(parent), kind, normal_ancestor_id, expn_id, span);
        self.arenas.alloc_module(module)
    }

    fn record_use(&mut self, ident: Ident, ns: Namespace,
                  used_binding: &'a NameBinding<'a>, is_lexical_scope: bool) {
        if let Some((b2, kind)) = used_binding.ambiguity {
            self.ambiguity_errors.push(AmbiguityError {
                kind, ident, b1: used_binding, b2,
                misc1: AmbiguityErrorMisc::None,
                misc2: AmbiguityErrorMisc::None,
            });
        }
        if let NameBindingKind::Import { directive, binding, ref used } = used_binding.kind {
            // Avoid marking `extern crate` items that refer to a name from extern prelude,
            // but not introduce it, as used if they are accessed from lexical scope.
            if is_lexical_scope {
                if let Some(entry) = self.extern_prelude.get(&ident.modern()) {
                    if let Some(crate_item) = entry.extern_crate_item {
                        if ptr::eq(used_binding, crate_item) && !entry.introduced_by_item {
                            return;
                        }
                    }
                }
            }
            used.set(true);
            directive.used.set(true);
            self.used_imports.insert((directive.id, ns));
            self.add_to_glob_map(&directive, ident);
            self.record_use(ident, ns, binding, false);
        }
    }

    #[inline]
    fn add_to_glob_map(&mut self, directive: &ImportDirective<'_>, ident: Ident) {
        if directive.is_glob() {
            self.glob_map.entry(directive.id).or_default().insert(ident.name);
        }
    }

    /// A generic scope visitor.
    /// Visits scopes in order to resolve some identifier in them or perform other actions.
    /// If the callback returns `Some` result, we stop visiting scopes and return it.
    fn visit_scopes<T>(
        &mut self,
        scope_set: ScopeSet,
        parent_scope: &ParentScope<'a>,
        ident: Ident,
        mut visitor: impl FnMut(&mut Self, Scope<'a>, /*use_prelude*/ bool, Ident) -> Option<T>,
    ) -> Option<T> {
        // General principles:
        // 1. Not controlled (user-defined) names should have higher priority than controlled names
        //    built into the language or standard library. This way we can add new names into the
        //    language or standard library without breaking user code.
        // 2. "Closed set" below means new names cannot appear after the current resolution attempt.
        // Places to search (in order of decreasing priority):
        // (Type NS)
        // 1. FIXME: Ribs (type parameters), there's no necessary infrastructure yet
        //    (open set, not controlled).
        // 2. Names in modules (both normal `mod`ules and blocks), loop through hygienic parents
        //    (open, not controlled).
        // 3. Extern prelude (open, the open part is from macro expansions, not controlled).
        // 4. Tool modules (closed, controlled right now, but not in the future).
        // 5. Standard library prelude (de-facto closed, controlled).
        // 6. Language prelude (closed, controlled).
        // (Value NS)
        // 1. FIXME: Ribs (local variables), there's no necessary infrastructure yet
        //    (open set, not controlled).
        // 2. Names in modules (both normal `mod`ules and blocks), loop through hygienic parents
        //    (open, not controlled).
        // 3. Standard library prelude (de-facto closed, controlled).
        // (Macro NS)
        // 1-3. Derive helpers (open, not controlled). All ambiguities with other names
        //    are currently reported as errors. They should be higher in priority than preludes
        //    and probably even names in modules according to the "general principles" above. They
        //    also should be subject to restricted shadowing because are effectively produced by
        //    derives (you need to resolve the derive first to add helpers into scope), but they
        //    should be available before the derive is expanded for compatibility.
        //    It's mess in general, so we are being conservative for now.
        // 1-3. `macro_rules` (open, not controlled), loop through legacy scopes. Have higher
        //    priority than prelude macros, but create ambiguities with macros in modules.
        // 1-3. Names in modules (both normal `mod`ules and blocks), loop through hygienic parents
        //    (open, not controlled). Have higher priority than prelude macros, but create
        //    ambiguities with `macro_rules`.
        // 4. `macro_use` prelude (open, the open part is from macro expansions, not controlled).
        // 4a. User-defined prelude from macro-use
        //    (open, the open part is from macro expansions, not controlled).
        // 4b. "Standard library prelude" part implemented through `macro-use` (closed, controlled).
        // 4c. Standard library prelude (de-facto closed, controlled).
        // 6. Language prelude: builtin attributes (closed, controlled).
        // 4-6. Legacy plugin helpers (open, not controlled). Similar to derive helpers,
        //    but introduced by legacy plugins using `register_attribute`. Priority is somewhere
        //    in prelude, not sure where exactly (creates ambiguities with any other prelude names).

        let rust_2015 = ident.span.rust_2015();
        let (ns, is_absolute_path) = match scope_set {
            ScopeSet::Import(ns) => (ns, false),
            ScopeSet::AbsolutePath(ns) => (ns, true),
            ScopeSet::Macro(_) => (MacroNS, false),
            ScopeSet::Module => (TypeNS, false),
        };
        let mut scope = match ns {
            _ if is_absolute_path => Scope::CrateRoot,
            TypeNS | ValueNS => Scope::Module(parent_scope.module),
            MacroNS => Scope::DeriveHelpers,
        };
        let mut ident = ident.modern();
        let mut use_prelude = !parent_scope.module.no_implicit_prelude;

        loop {
            let visit = match scope {
                Scope::DeriveHelpers => true,
                Scope::MacroRules(..) => true,
                Scope::CrateRoot => true,
                Scope::Module(..) => true,
                Scope::MacroUsePrelude => use_prelude || rust_2015,
                Scope::BuiltinAttrs => true,
                Scope::LegacyPluginHelpers => use_prelude || rust_2015,
                Scope::ExternPrelude => use_prelude || is_absolute_path,
                Scope::ToolPrelude => use_prelude,
                Scope::StdLibPrelude => use_prelude || ns == MacroNS,
                Scope::BuiltinTypes => true,
            };

            if visit {
                if let break_result @ Some(..) = visitor(self, scope, use_prelude, ident) {
                    return break_result;
                }
            }

            scope = match scope {
                Scope::DeriveHelpers =>
                    Scope::MacroRules(parent_scope.legacy),
                Scope::MacroRules(legacy_scope) => match legacy_scope {
                    LegacyScope::Binding(binding) => Scope::MacroRules(
                        binding.parent_legacy_scope
                    ),
                    LegacyScope::Invocation(invoc) => Scope::MacroRules(
                        invoc.output_legacy_scope.get().unwrap_or(invoc.parent_legacy_scope)
                    ),
                    LegacyScope::Empty => Scope::Module(parent_scope.module),
                }
                Scope::CrateRoot => match ns {
                    TypeNS => {
                        ident.span.adjust(ExpnId::root());
                        Scope::ExternPrelude
                    }
                    ValueNS | MacroNS => break,
                }
                Scope::Module(module) => {
                    use_prelude = !module.no_implicit_prelude;
                    match self.hygienic_lexical_parent(module, &mut ident.span) {
                        Some(parent_module) => Scope::Module(parent_module),
                        None => {
                            ident.span.adjust(ExpnId::root());
                            match ns {
                                TypeNS => Scope::ExternPrelude,
                                ValueNS => Scope::StdLibPrelude,
                                MacroNS => Scope::MacroUsePrelude,
                            }
                        }
                    }
                }
                Scope::MacroUsePrelude => Scope::StdLibPrelude,
                Scope::BuiltinAttrs => Scope::LegacyPluginHelpers,
                Scope::LegacyPluginHelpers => break, // nowhere else to search
                Scope::ExternPrelude if is_absolute_path => break,
                Scope::ExternPrelude => Scope::ToolPrelude,
                Scope::ToolPrelude => Scope::StdLibPrelude,
                Scope::StdLibPrelude => match ns {
                    TypeNS => Scope::BuiltinTypes,
                    ValueNS => break, // nowhere else to search
                    MacroNS => Scope::BuiltinAttrs,
                }
                Scope::BuiltinTypes => break, // nowhere else to search
            };
        }

        None
    }

    /// This resolves the identifier `ident` in the namespace `ns` in the current lexical scope.
    /// More specifically, we proceed up the hierarchy of scopes and return the binding for
    /// `ident` in the first scope that defines it (or None if no scopes define it).
    ///
    /// A block's items are above its local variables in the scope hierarchy, regardless of where
    /// the items are defined in the block. For example,
    /// ```rust
    /// fn f() {
    ///    g(); // Since there are no local variables in scope yet, this resolves to the item.
    ///    let g = || {};
    ///    fn g() {}
    ///    g(); // This resolves to the local variable `g` since it shadows the item.
    /// }
    /// ```
    ///
    /// Invariant: This must only be called during main resolution, not during
    /// import resolution.
    fn resolve_ident_in_lexical_scope(&mut self,
                                      mut ident: Ident,
                                      ns: Namespace,
                                      parent_scope: &ParentScope<'a>,
                                      record_used_id: Option<NodeId>,
                                      path_span: Span,
                                      ribs: &[Rib<'a>])
                                      -> Option<LexicalScopeBinding<'a>> {
        assert!(ns == TypeNS || ns == ValueNS);
        if ident.name == kw::Invalid {
            return Some(LexicalScopeBinding::Res(Res::Err));
        }
        let (general_span, modern_span) = if ident.name == kw::SelfUpper {
            // FIXME(jseyfried) improve `Self` hygiene
            let empty_span = ident.span.with_ctxt(SyntaxContext::empty());
            (empty_span, empty_span)
        } else if ns == TypeNS {
            let modern_span = ident.span.modern();
            (modern_span, modern_span)
        } else {
            (ident.span.modern_and_legacy(), ident.span.modern())
        };
        ident.span = general_span;
        let modern_ident = Ident { span: modern_span, ..ident };

        // Walk backwards up the ribs in scope.
        let record_used = record_used_id.is_some();
        let mut module = self.graph_root;
        for i in (0 .. ribs.len()).rev() {
            debug!("walk rib\n{:?}", ribs[i].bindings);
            // Use the rib kind to determine whether we are resolving parameters
            // (modern hygiene) or local variables (legacy hygiene).
            let rib_ident = if let AssocItemRibKind | ItemRibKind = ribs[i].kind {
                modern_ident
            } else {
                ident
            };
            if let Some(res) = ribs[i].bindings.get(&rib_ident).cloned() {
                // The ident resolves to a type parameter or local variable.
                return Some(LexicalScopeBinding::Res(
                    self.validate_res_from_ribs(i, res, record_used, path_span, ribs),
                ));
            }

            module = match ribs[i].kind {
                ModuleRibKind(module) => module,
                MacroDefinition(def) if def == self.macro_def(ident.span.ctxt()) => {
                    // If an invocation of this macro created `ident`, give up on `ident`
                    // and switch to `ident`'s source from the macro definition.
                    ident.span.remove_mark();
                    continue
                }
                _ => continue,
            };


            let item = self.resolve_ident_in_module_unadjusted(
                ModuleOrUniformRoot::Module(module),
                ident,
                ns,
                parent_scope,
                record_used,
                path_span,
            );
            if let Ok(binding) = item {
                // The ident resolves to an item.
                return Some(LexicalScopeBinding::Item(binding));
            }

            match module.kind {
                ModuleKind::Block(..) => {}, // We can see through blocks
                _ => break,
            }
        }

        ident = modern_ident;
        let mut poisoned = None;
        loop {
            let opt_module = if let Some(node_id) = record_used_id {
                self.hygienic_lexical_parent_with_compatibility_fallback(module, &mut ident.span,
                                                                         node_id, &mut poisoned)
            } else {
                self.hygienic_lexical_parent(module, &mut ident.span)
            };
            module = unwrap_or!(opt_module, break);
            let adjusted_parent_scope = &ParentScope { module, ..parent_scope.clone() };
            let result = self.resolve_ident_in_module_unadjusted(
                ModuleOrUniformRoot::Module(module),
                ident,
                ns,
                adjusted_parent_scope,
                record_used,
                path_span,
            );

            match result {
                Ok(binding) => {
                    if let Some(node_id) = poisoned {
                        self.session.buffer_lint_with_diagnostic(
                            lint::builtin::PROC_MACRO_DERIVE_RESOLUTION_FALLBACK,
                            node_id, ident.span,
                            &format!("cannot find {} `{}` in this scope", ns.descr(), ident),
                            lint::builtin::BuiltinLintDiagnostics::
                                ProcMacroDeriveResolutionFallback(ident.span),
                        );
                    }
                    return Some(LexicalScopeBinding::Item(binding))
                }
                Err(Determined) => continue,
                Err(Undetermined) =>
                    span_bug!(ident.span, "undetermined resolution during main resolution pass"),
            }
        }

        if !module.no_implicit_prelude {
            ident.span.adjust(ExpnId::root());
            if ns == TypeNS {
                if let Some(binding) = self.extern_prelude_get(ident, !record_used) {
                    return Some(LexicalScopeBinding::Item(binding));
                }
            }
            if ns == TypeNS && KNOWN_TOOLS.contains(&ident.name) {
                let binding = (Res::ToolMod, ty::Visibility::Public,
                               DUMMY_SP, ExpnId::root()).to_name_binding(self.arenas);
                return Some(LexicalScopeBinding::Item(binding));
            }
            if let Some(prelude) = self.prelude {
                if let Ok(binding) = self.resolve_ident_in_module_unadjusted(
                    ModuleOrUniformRoot::Module(prelude),
                    ident,
                    ns,
                    parent_scope,
                    false,
                    path_span,
                ) {
                    return Some(LexicalScopeBinding::Item(binding));
                }
            }
        }

        None
    }

    fn hygienic_lexical_parent(&mut self, module: Module<'a>, span: &mut Span)
                               -> Option<Module<'a>> {
        if !module.expansion.outer_expn_is_descendant_of(span.ctxt()) {
            return Some(self.macro_def_scope(span.remove_mark()));
        }

        if let ModuleKind::Block(..) = module.kind {
            return Some(module.parent.unwrap());
        }

        None
    }

    fn hygienic_lexical_parent_with_compatibility_fallback(&mut self, module: Module<'a>,
                                                           span: &mut Span, node_id: NodeId,
                                                           poisoned: &mut Option<NodeId>)
                                                           -> Option<Module<'a>> {
        if let module @ Some(..) = self.hygienic_lexical_parent(module, span) {
            return module;
        }

        // We need to support the next case under a deprecation warning
        // ```
        // struct MyStruct;
        // ---- begin: this comes from a proc macro derive
        // mod implementation_details {
        //     // Note that `MyStruct` is not in scope here.
        //     impl SomeTrait for MyStruct { ... }
        // }
        // ---- end
        // ```
        // So we have to fall back to the module's parent during lexical resolution in this case.
        if let Some(parent) = module.parent {
            // Inner module is inside the macro, parent module is outside of the macro.
            if module.expansion != parent.expansion &&
            module.expansion.is_descendant_of(parent.expansion) {
                // The macro is a proc macro derive
                if module.expansion.looks_like_proc_macro_derive() {
                    if parent.expansion.outer_expn_is_descendant_of(span.ctxt()) {
                        *poisoned = Some(node_id);
                        return module.parent;
                    }
                }
            }
        }

        None
    }

    fn resolve_ident_in_module(
        &mut self,
        module: ModuleOrUniformRoot<'a>,
        ident: Ident,
        ns: Namespace,
        parent_scope: &ParentScope<'a>,
        record_used: bool,
        path_span: Span
    ) -> Result<&'a NameBinding<'a>, Determinacy> {
        self.resolve_ident_in_module_ext(
            module, ident, ns, parent_scope, record_used, path_span
        ).map_err(|(determinacy, _)| determinacy)
    }

    fn resolve_ident_in_module_ext(
        &mut self,
        module: ModuleOrUniformRoot<'a>,
        mut ident: Ident,
        ns: Namespace,
        parent_scope: &ParentScope<'a>,
        record_used: bool,
        path_span: Span
    ) -> Result<&'a NameBinding<'a>, (Determinacy, Weak)> {
        let tmp_parent_scope;
        let mut adjusted_parent_scope = parent_scope;
        match module {
            ModuleOrUniformRoot::Module(m) => {
                if let Some(def) = ident.span.modernize_and_adjust(m.expansion) {
                    tmp_parent_scope =
                        ParentScope { module: self.macro_def_scope(def), ..parent_scope.clone() };
                    adjusted_parent_scope = &tmp_parent_scope;
                }
            }
            ModuleOrUniformRoot::ExternPrelude => {
                ident.span.modernize_and_adjust(ExpnId::root());
            }
            ModuleOrUniformRoot::CrateRootAndExternPrelude |
            ModuleOrUniformRoot::CurrentScope => {
                // No adjustments
            }
        }
        let result = self.resolve_ident_in_module_unadjusted_ext(
            module, ident, ns, adjusted_parent_scope, false, record_used, path_span,
        );
        result
    }

    fn resolve_crate_root(&mut self, ident: Ident) -> Module<'a> {
        let mut ctxt = ident.span.ctxt();
        let mark = if ident.name == kw::DollarCrate {
            // When resolving `$crate` from a `macro_rules!` invoked in a `macro`,
            // we don't want to pretend that the `macro_rules!` definition is in the `macro`
            // as described in `SyntaxContext::apply_mark`, so we ignore prepended modern marks.
            // FIXME: This is only a guess and it doesn't work correctly for `macro_rules!`
            // definitions actually produced by `macro` and `macro` definitions produced by
            // `macro_rules!`, but at least such configurations are not stable yet.
            ctxt = ctxt.modern_and_legacy();
            let mut iter = ctxt.marks().into_iter().rev().peekable();
            let mut result = None;
            // Find the last modern mark from the end if it exists.
            while let Some(&(mark, transparency)) = iter.peek() {
                if transparency == Transparency::Opaque {
                    result = Some(mark);
                    iter.next();
                } else {
                    break;
                }
            }
            // Then find the last legacy mark from the end if it exists.
            for (mark, transparency) in iter {
                if transparency == Transparency::SemiTransparent {
                    result = Some(mark);
                } else {
                    break;
                }
            }
            result
        } else {
            ctxt = ctxt.modern();
            ctxt.adjust(ExpnId::root())
        };
        let module = match mark {
            Some(def) => self.macro_def_scope(def),
            None => return self.graph_root,
        };
        self.get_module(DefId { index: CRATE_DEF_INDEX, ..module.normal_ancestor_id })
    }

    fn resolve_self(&mut self, ctxt: &mut SyntaxContext, module: Module<'a>) -> Module<'a> {
        let mut module = self.get_module(module.normal_ancestor_id);
        while module.span.ctxt().modern() != *ctxt {
            let parent = module.parent.unwrap_or_else(|| self.macro_def_scope(ctxt.remove_mark()));
            module = self.get_module(parent.normal_ancestor_id);
        }
        module
    }

    fn resolve_path(
        &mut self,
        path: &[Segment],
        opt_ns: Option<Namespace>, // `None` indicates a module path in import
        parent_scope: &ParentScope<'a>,
        record_used: bool,
        path_span: Span,
        crate_lint: CrateLint,
    ) -> PathResult<'a> {
        self.resolve_path_with_ribs(
            path, opt_ns, parent_scope, record_used, path_span, crate_lint, &Default::default()
        )
    }

    fn resolve_path_with_ribs(
        &mut self,
        path: &[Segment],
        opt_ns: Option<Namespace>, // `None` indicates a module path in import
        parent_scope: &ParentScope<'a>,
        record_used: bool,
        path_span: Span,
        crate_lint: CrateLint,
        ribs: &PerNS<Vec<Rib<'a>>>,
    ) -> PathResult<'a> {
        let mut module = None;
        let mut allow_super = true;
        let mut second_binding = None;

        debug!(
            "resolve_path(path={:?}, opt_ns={:?}, record_used={:?}, \
             path_span={:?}, crate_lint={:?})",
            path,
            opt_ns,
            record_used,
            path_span,
            crate_lint,
        );

        for (i, &Segment { ident, id }) in path.iter().enumerate() {
            debug!("resolve_path ident {} {:?} {:?}", i, ident, id);
            let record_segment_res = |this: &mut Self, res| {
                if record_used {
                    if let Some(id) = id {
                        if !this.partial_res_map.contains_key(&id) {
                            assert!(id != ast::DUMMY_NODE_ID, "Trying to resolve dummy id");
                            this.record_partial_res(id, PartialRes::new(res));
                        }
                    }
                }
            };

            let is_last = i == path.len() - 1;
            let ns = if is_last { opt_ns.unwrap_or(TypeNS) } else { TypeNS };
            let name = ident.name;

            allow_super &= ns == TypeNS &&
                (name == kw::SelfLower ||
                 name == kw::Super);

            if ns == TypeNS {
                if allow_super && name == kw::Super {
                    let mut ctxt = ident.span.ctxt().modern();
                    let self_module = match i {
                        0 => Some(self.resolve_self(&mut ctxt, parent_scope.module)),
                        _ => match module {
                            Some(ModuleOrUniformRoot::Module(module)) => Some(module),
                            _ => None,
                        },
                    };
                    if let Some(self_module) = self_module {
                        if let Some(parent) = self_module.parent {
                            module = Some(ModuleOrUniformRoot::Module(
                                self.resolve_self(&mut ctxt, parent)));
                            continue;
                        }
                    }
                    let msg = "there are too many initial `super`s.".to_string();
                    return PathResult::Failed {
                        span: ident.span,
                        label: msg,
                        suggestion: None,
                        is_error_from_last_segment: false,
                    };
                }
                if i == 0 {
                    if name == kw::SelfLower {
                        let mut ctxt = ident.span.ctxt().modern();
                        module = Some(ModuleOrUniformRoot::Module(
                            self.resolve_self(&mut ctxt, parent_scope.module)));
                        continue;
                    }
                    if name == kw::PathRoot && ident.span.rust_2018() {
                        module = Some(ModuleOrUniformRoot::ExternPrelude);
                        continue;
                    }
                    if name == kw::PathRoot &&
                       ident.span.rust_2015() && self.session.rust_2018() {
                        // `::a::b` from 2015 macro on 2018 global edition
                        module = Some(ModuleOrUniformRoot::CrateRootAndExternPrelude);
                        continue;
                    }
                    if name == kw::PathRoot ||
                       name == kw::Crate ||
                       name == kw::DollarCrate {
                        // `::a::b`, `crate::a::b` or `$crate::a::b`
                        module = Some(ModuleOrUniformRoot::Module(
                            self.resolve_crate_root(ident)));
                        continue;
                    }
                }
            }

            // Report special messages for path segment keywords in wrong positions.
            if ident.is_path_segment_keyword() && i != 0 {
                let name_str = if name == kw::PathRoot {
                    "crate root".to_string()
                } else {
                    format!("`{}`", name)
                };
                let label = if i == 1 && path[0].ident.name == kw::PathRoot {
                    format!("global paths cannot start with {}", name_str)
                } else {
                    format!("{} in paths can only be used in start position", name_str)
                };
                return PathResult::Failed {
                    span: ident.span,
                    label,
                    suggestion: None,
                    is_error_from_last_segment: false,
                };
            }

            let binding = if let Some(module) = module {
                self.resolve_ident_in_module(
                    module, ident, ns, parent_scope, record_used, path_span
                )
            } else if opt_ns.is_none() || opt_ns == Some(MacroNS) {
                assert!(ns == TypeNS);
                let scopes = if opt_ns.is_none() { ScopeSet::Import(ns) } else { ScopeSet::Module };
                self.early_resolve_ident_in_lexical_scope(ident, scopes, parent_scope, record_used,
                                                          record_used, path_span)
            } else {
                let record_used_id =
                    if record_used { crate_lint.node_id().or(Some(CRATE_NODE_ID)) } else { None };
                match self.resolve_ident_in_lexical_scope(
                    ident, ns, parent_scope, record_used_id, path_span, &ribs[ns]
                ) {
                    // we found a locally-imported or available item/module
                    Some(LexicalScopeBinding::Item(binding)) => Ok(binding),
                    // we found a local variable or type param
                    Some(LexicalScopeBinding::Res(res))
                            if opt_ns == Some(TypeNS) || opt_ns == Some(ValueNS) => {
                        record_segment_res(self, res);
                        return PathResult::NonModule(PartialRes::with_unresolved_segments(
                            res, path.len() - 1
                        ));
                    }
                    _ => Err(Determinacy::determined(record_used)),
                }
            };

            match binding {
                Ok(binding) => {
                    if i == 1 {
                        second_binding = Some(binding);
                    }
                    let res = binding.res();
                    let maybe_assoc = opt_ns != Some(MacroNS) && PathSource::Type.is_expected(res);
                    if let Some(next_module) = binding.module() {
                        module = Some(ModuleOrUniformRoot::Module(next_module));
                        record_segment_res(self, res);
                    } else if res == Res::ToolMod && i + 1 != path.len() {
                        if binding.is_import() {
                            self.session.struct_span_err(
                                ident.span, "cannot use a tool module through an import"
                            ).span_note(
                                binding.span, "the tool module imported here"
                            ).emit();
                        }
                        let res = Res::NonMacroAttr(NonMacroAttrKind::Tool);
                        return PathResult::NonModule(PartialRes::new(res));
                    } else if res == Res::Err {
                        return PathResult::NonModule(PartialRes::new(Res::Err));
                    } else if opt_ns.is_some() && (is_last || maybe_assoc) {
                        self.lint_if_path_starts_with_module(
                            crate_lint,
                            path,
                            path_span,
                            second_binding,
                        );
                        return PathResult::NonModule(PartialRes::with_unresolved_segments(
                            res, path.len() - i - 1
                        ));
                    } else {
                        let label = format!(
                            "`{}` is {} {}, not a module",
                            ident,
                            res.article(),
                            res.descr(),
                        );

                        return PathResult::Failed {
                            span: ident.span,
                            label,
                            suggestion: None,
                            is_error_from_last_segment: is_last,
                        };
                    }
                }
                Err(Undetermined) => return PathResult::Indeterminate,
                Err(Determined) => {
                    if let Some(ModuleOrUniformRoot::Module(module)) = module {
                        if opt_ns.is_some() && !module.is_normal() {
                            return PathResult::NonModule(PartialRes::with_unresolved_segments(
                                module.res().unwrap(), path.len() - i
                            ));
                        }
                    }
                    let module_res = match module {
                        Some(ModuleOrUniformRoot::Module(module)) => module.res(),
                        _ => None,
                    };
                    let (label, suggestion) = if module_res == self.graph_root.res() {
                        let is_mod = |res| {
                            match res { Res::Def(DefKind::Mod, _) => true, _ => false }
                        };
                        let mut candidates =
                            self.lookup_import_candidates(ident, TypeNS, is_mod);
                        candidates.sort_by_cached_key(|c| {
                            (c.path.segments.len(), c.path.to_string())
                        });
                        if let Some(candidate) = candidates.get(0) {
                            (
                                String::from("unresolved import"),
                                Some((
                                    vec![(ident.span, candidate.path.to_string())],
                                    String::from("a similar path exists"),
                                    Applicability::MaybeIncorrect,
                                )),
                            )
                        } else if !ident.is_reserved() {
                            (format!("maybe a missing crate `{}`?", ident), None)
                        } else {
                            // the parser will already have complained about the keyword being used
                            return PathResult::NonModule(PartialRes::new(Res::Err));
                        }
                    } else if i == 0 {
                        (format!("use of undeclared type or module `{}`", ident), None)
                    } else {
                        (format!("could not find `{}` in `{}`", ident, path[i - 1].ident), None)
                    };
                    return PathResult::Failed {
                        span: ident.span,
                        label,
                        suggestion,
                        is_error_from_last_segment: is_last,
                    };
                }
            }
        }

        self.lint_if_path_starts_with_module(crate_lint, path, path_span, second_binding);

        PathResult::Module(match module {
            Some(module) => module,
            None if path.is_empty() => ModuleOrUniformRoot::CurrentScope,
            _ => span_bug!(path_span, "resolve_path: non-empty path `{:?}` has no module", path),
        })
    }

    fn lint_if_path_starts_with_module(
        &self,
        crate_lint: CrateLint,
        path: &[Segment],
        path_span: Span,
        second_binding: Option<&NameBinding<'_>>,
    ) {
        let (diag_id, diag_span) = match crate_lint {
            CrateLint::No => return,
            CrateLint::SimplePath(id) => (id, path_span),
            CrateLint::UsePath { root_id, root_span } => (root_id, root_span),
            CrateLint::QPathTrait { qpath_id, qpath_span } => (qpath_id, qpath_span),
        };

        let first_name = match path.get(0) {
            // In the 2018 edition this lint is a hard error, so nothing to do
            Some(seg) if seg.ident.span.rust_2015() && self.session.rust_2015() => seg.ident.name,
            _ => return,
        };

        // We're only interested in `use` paths which should start with
        // `{{root}}` currently.
        if first_name != kw::PathRoot {
            return
        }

        match path.get(1) {
            // If this import looks like `crate::...` it's already good
            Some(Segment { ident, .. }) if ident.name == kw::Crate => return,
            // Otherwise go below to see if it's an extern crate
            Some(_) => {}
            // If the path has length one (and it's `PathRoot` most likely)
            // then we don't know whether we're gonna be importing a crate or an
            // item in our crate. Defer this lint to elsewhere
            None => return,
        }

        // If the first element of our path was actually resolved to an
        // `ExternCrate` (also used for `crate::...`) then no need to issue a
        // warning, this looks all good!
        if let Some(binding) = second_binding {
            if let NameBindingKind::Import { directive: d, .. } = binding.kind {
                // Careful: we still want to rewrite paths from
                // renamed extern crates.
                if let ImportDirectiveSubclass::ExternCrate { source: None, .. } = d.subclass {
                    return
                }
            }
        }

        let diag = lint::builtin::BuiltinLintDiagnostics
            ::AbsPathWithModule(diag_span);
        self.session.buffer_lint_with_diagnostic(
            lint::builtin::ABSOLUTE_PATHS_NOT_STARTING_WITH_CRATE,
            diag_id, diag_span,
            "absolute paths must start with `self`, `super`, \
            `crate`, or an external crate name in the 2018 edition",
            diag);
    }

    // Validate a local resolution (from ribs).
    fn validate_res_from_ribs(
        &mut self,
        rib_index: usize,
        res: Res,
        record_used: bool,
        span: Span,
        all_ribs: &[Rib<'a>],
    ) -> Res {
        debug!("validate_res_from_ribs({:?})", res);
        let ribs = &all_ribs[rib_index + 1..];

        // An invalid forward use of a type parameter from a previous default.
        if let ForwardTyParamBanRibKind = all_ribs[rib_index].kind {
            if record_used {
                resolve_error(self, span, ResolutionError::ForwardDeclaredTyParam);
            }
            assert_eq!(res, Res::Err);
            return Res::Err;
        }

        // An invalid use of a type parameter as the type of a const parameter.
        if let TyParamAsConstParamTy = all_ribs[rib_index].kind {
            if record_used {
                resolve_error(self, span, ResolutionError::ConstParamDependentOnTypeParam);
            }
            assert_eq!(res, Res::Err);
            return Res::Err;
        }

        match res {
            Res::Local(_) => {
                use ResolutionError::*;
                let mut res_err = None;

                for rib in ribs {
                    match rib.kind {
                        NormalRibKind | ModuleRibKind(..) | MacroDefinition(..) |
                        ForwardTyParamBanRibKind | TyParamAsConstParamTy => {
                            // Nothing to do. Continue.
                        }
                        ItemRibKind | FnItemRibKind | AssocItemRibKind => {
                            // This was an attempt to access an upvar inside a
                            // named function item. This is not allowed, so we
                            // report an error.
                            if record_used {
                                // We don't immediately trigger a resolve error, because
                                // we want certain other resolution errors (namely those
                                // emitted for `ConstantItemRibKind` below) to take
                                // precedence.
                                res_err = Some(CannotCaptureDynamicEnvironmentInFnItem);
                            }
                        }
                        ConstantItemRibKind => {
                            // Still doesn't deal with upvars
                            if record_used {
                                resolve_error(self, span, AttemptToUseNonConstantValueInConstant);
                            }
                            return Res::Err;
                        }
                    }
                }
                if let Some(res_err) = res_err {
                     resolve_error(self, span, res_err);
                     return Res::Err;
                }
            }
            Res::Def(DefKind::TyParam, _) | Res::SelfTy(..) => {
                for rib in ribs {
                    match rib.kind {
                        NormalRibKind | AssocItemRibKind |
                        ModuleRibKind(..) | MacroDefinition(..) | ForwardTyParamBanRibKind |
                        ConstantItemRibKind | TyParamAsConstParamTy => {
                            // Nothing to do. Continue.
                        }
                        ItemRibKind | FnItemRibKind => {
                            // This was an attempt to use a type parameter outside its scope.
                            if record_used {
                                resolve_error(
                                    self,
                                    span,
                                    ResolutionError::GenericParamsFromOuterFunction(res),
                                );
                            }
                            return Res::Err;
                        }
                    }
                }
            }
            Res::Def(DefKind::ConstParam, _) => {
                let mut ribs = ribs.iter().peekable();
                if let Some(Rib { kind: FnItemRibKind, .. }) = ribs.peek() {
                    // When declaring const parameters inside function signatures, the first rib
                    // is always a `FnItemRibKind`. In this case, we can skip it, to avoid it
                    // (spuriously) conflicting with the const param.
                    ribs.next();
                }
                for rib in ribs {
                    if let ItemRibKind | FnItemRibKind = rib.kind {
                        // This was an attempt to use a const parameter outside its scope.
                        if record_used {
                            resolve_error(
                                self,
                                span,
                                ResolutionError::GenericParamsFromOuterFunction(res),
                            );
                        }
                        return Res::Err;
                    }
                }
            }
            _ => {}
        }
        res
    }

    fn record_partial_res(&mut self, node_id: NodeId, resolution: PartialRes) {
        debug!("(recording res) recording {:?} for {}", resolution, node_id);
        if let Some(prev_res) = self.partial_res_map.insert(node_id, resolution) {
            panic!("path resolved multiple times ({:?} before, {:?} now)", prev_res, resolution);
        }
    }

    fn resolve_visibility(
        &mut self, vis: &ast::Visibility, parent_scope: &ParentScope<'a>
    ) -> ty::Visibility {
        match vis.node {
            ast::VisibilityKind::Public => ty::Visibility::Public,
            ast::VisibilityKind::Crate(..) => {
                ty::Visibility::Restricted(DefId::local(CRATE_DEF_INDEX))
            }
            ast::VisibilityKind::Inherited => {
                ty::Visibility::Restricted(parent_scope.module.normal_ancestor_id)
            }
            ast::VisibilityKind::Restricted { ref path, id, .. } => {
                // For visibilities we are not ready to provide correct implementation of "uniform
                // paths" right now, so on 2018 edition we only allow module-relative paths for now.
                // On 2015 edition visibilities are resolved as crate-relative by default,
                // so we are prepending a root segment if necessary.
                let ident = path.segments.get(0).expect("empty path in visibility").ident;
                let crate_root = if ident.is_path_segment_keyword() {
                    None
                } else if ident.span.rust_2018() {
                    let msg = "relative paths are not supported in visibilities on 2018 edition";
                    self.session.struct_span_err(ident.span, msg)
                        .span_suggestion(
                            path.span,
                            "try",
                            format!("crate::{}", path),
                            Applicability::MaybeIncorrect,
                        )
                        .emit();
                    return ty::Visibility::Public;
                } else {
                    let ctxt = ident.span.ctxt();
                    Some(Segment::from_ident(Ident::new(
                        kw::PathRoot, path.span.shrink_to_lo().with_ctxt(ctxt)
                    )))
                };

                let segments = crate_root.into_iter()
                    .chain(path.segments.iter().map(|seg| seg.into())).collect::<Vec<_>>();
                let expected_found_error = |this: &Self, res: Res| {
                    let path_str = Segment::names_to_string(&segments);
                    struct_span_err!(this.session, path.span, E0577,
                                     "expected module, found {} `{}`", res.descr(), path_str)
                        .span_label(path.span, "not a module").emit();
                };
                match self.resolve_path(
                    &segments,
                    Some(TypeNS),
                    parent_scope,
                    true,
                    path.span,
                    CrateLint::SimplePath(id),
                ) {
                    PathResult::Module(ModuleOrUniformRoot::Module(module)) => {
                        let res = module.res().expect("visibility resolved to unnamed block");
                        self.record_partial_res(id, PartialRes::new(res));
                        if module.is_normal() {
                            if res == Res::Err {
                                ty::Visibility::Public
                            } else {
                                let vis = ty::Visibility::Restricted(res.def_id());
                                if self.is_accessible_from(vis, parent_scope.module) {
                                    vis
                                } else {
                                    let msg =
                                        "visibilities can only be restricted to ancestor modules";
                                    self.session.span_err(path.span, msg);
                                    ty::Visibility::Public
                                }
                            }
                        } else {
                            expected_found_error(self, res);
                            ty::Visibility::Public
                        }
                    }
                    PathResult::Module(..) => {
                        self.session.span_err(path.span, "visibility must resolve to a module");
                        ty::Visibility::Public
                    }
                    PathResult::NonModule(partial_res) => {
                        expected_found_error(self, partial_res.base_res());
                        ty::Visibility::Public
                    }
                    PathResult::Failed { span, label, suggestion, .. } => {
                        let err = ResolutionError::FailedToResolve { label, suggestion };
                        resolve_error(self, span, err);
                        ty::Visibility::Public
                    }
                    PathResult::Indeterminate => {
                        span_err!(self.session, path.span, E0578,
                                  "cannot determine resolution for the visibility");
                        ty::Visibility::Public
                    }
                }
            }
        }
    }

    fn is_accessible_from(&self, vis: ty::Visibility, module: Module<'a>) -> bool {
        vis.is_accessible_from(module.normal_ancestor_id, self)
    }

    fn set_binding_parent_module(&mut self, binding: &'a NameBinding<'a>, module: Module<'a>) {
        if let Some(old_module) = self.binding_parent_modules.insert(PtrKey(binding), module) {
            if !ptr::eq(module, old_module) {
                span_bug!(binding.span, "parent module is reset for binding");
            }
        }
    }

    fn disambiguate_legacy_vs_modern(
        &self,
        legacy: &'a NameBinding<'a>,
        modern: &'a NameBinding<'a>,
    ) -> bool {
        // Some non-controversial subset of ambiguities "modern macro name" vs "macro_rules"
        // is disambiguated to mitigate regressions from macro modularization.
        // Scoping for `macro_rules` behaves like scoping for `let` at module level, in general.
        match (self.binding_parent_modules.get(&PtrKey(legacy)),
               self.binding_parent_modules.get(&PtrKey(modern))) {
            (Some(legacy), Some(modern)) =>
                legacy.normal_ancestor_id == modern.normal_ancestor_id &&
                modern.is_ancestor_of(legacy),
            _ => false,
        }
    }

    fn binding_description(&self, b: &NameBinding<'_>, ident: Ident, from_prelude: bool) -> String {
        if b.span.is_dummy() {
            let add_built_in = match b.res() {
                // These already contain the "built-in" prefix or look bad with it.
                Res::NonMacroAttr(..) | Res::PrimTy(..) | Res::ToolMod => false,
                _ => true,
            };
            let (built_in, from) = if from_prelude {
                ("", " from prelude")
            } else if b.is_extern_crate() && !b.is_import() &&
                        self.session.opts.externs.get(&ident.as_str()).is_some() {
                ("", " passed with `--extern`")
            } else if add_built_in {
                (" built-in", "")
            } else {
                ("", "")
            };

            let article = if built_in.is_empty() { b.article() } else { "a" };
            format!("{a}{built_in} {thing}{from}",
                    a = article, thing = b.descr(), built_in = built_in, from = from)
        } else {
            let introduced = if b.is_import() { "imported" } else { "defined" };
            format!("the {thing} {introduced} here",
                    thing = b.descr(), introduced = introduced)
        }
    }

    fn report_ambiguity_error(&self, ambiguity_error: &AmbiguityError<'_>) {
        let AmbiguityError { kind, ident, b1, b2, misc1, misc2 } = *ambiguity_error;
        let (b1, b2, misc1, misc2, swapped) = if b2.span.is_dummy() && !b1.span.is_dummy() {
            // We have to print the span-less alternative first, otherwise formatting looks bad.
            (b2, b1, misc2, misc1, true)
        } else {
            (b1, b2, misc1, misc2, false)
        };

        let mut err = struct_span_err!(self.session, ident.span, E0659,
                                       "`{ident}` is ambiguous ({why})",
                                       ident = ident, why = kind.descr());
        err.span_label(ident.span, "ambiguous name");

        let mut could_refer_to = |b: &NameBinding<'_>, misc: AmbiguityErrorMisc, also: &str| {
            let what = self.binding_description(b, ident, misc == AmbiguityErrorMisc::FromPrelude);
            let note_msg = format!("`{ident}` could{also} refer to {what}",
                                   ident = ident, also = also, what = what);

            let mut help_msgs = Vec::new();
            if b.is_glob_import() && (kind == AmbiguityKind::GlobVsGlob ||
                                      kind == AmbiguityKind::GlobVsExpanded ||
                                      kind == AmbiguityKind::GlobVsOuter &&
                                      swapped != also.is_empty()) {
                help_msgs.push(format!("consider adding an explicit import of \
                                        `{ident}` to disambiguate", ident = ident))
            }
            if b.is_extern_crate() && ident.span.rust_2018() {
                help_msgs.push(format!(
                    "use `::{ident}` to refer to this {thing} unambiguously",
                    ident = ident, thing = b.descr(),
                ))
            }
            if misc == AmbiguityErrorMisc::SuggestCrate {
                help_msgs.push(format!(
                    "use `crate::{ident}` to refer to this {thing} unambiguously",
                    ident = ident, thing = b.descr(),
                ))
            } else if misc == AmbiguityErrorMisc::SuggestSelf {
                help_msgs.push(format!(
                    "use `self::{ident}` to refer to this {thing} unambiguously",
                    ident = ident, thing = b.descr(),
                ))
            }

            err.span_note(b.span, &note_msg);
            for (i, help_msg) in help_msgs.iter().enumerate() {
                let or = if i == 0 { "" } else { "or " };
                err.help(&format!("{}{}", or, help_msg));
            }
        };

        could_refer_to(b1, misc1, "");
        could_refer_to(b2, misc2, " also");
        err.emit();
    }

    fn report_errors(&mut self, krate: &Crate) {
        self.report_with_use_injections(krate);

        for &(span_use, span_def) in &self.macro_expanded_macro_export_errors {
            let msg = "macro-expanded `macro_export` macros from the current crate \
                       cannot be referred to by absolute paths";
            self.session.buffer_lint_with_diagnostic(
                lint::builtin::MACRO_EXPANDED_MACRO_EXPORTS_ACCESSED_BY_ABSOLUTE_PATHS,
                CRATE_NODE_ID, span_use, msg,
                lint::builtin::BuiltinLintDiagnostics::
                    MacroExpandedMacroExportsAccessedByAbsolutePaths(span_def),
            );
        }

        for ambiguity_error in &self.ambiguity_errors {
            self.report_ambiguity_error(ambiguity_error);
        }

        let mut reported_spans = FxHashSet::default();
        for &PrivacyError(dedup_span, ident, binding) in &self.privacy_errors {
            if reported_spans.insert(dedup_span) {
                let mut err = struct_span_err!(
                    self.session,
                    ident.span,
                    E0603,
                    "{} `{}` is private",
                    binding.descr(),
                    ident.name,
                );
                // FIXME: use the ctor's `def_id` to check wether any of the fields is not visible
                match binding.kind {
                    NameBindingKind::Res(Res::Def(DefKind::Ctor(
                        CtorOf::Struct,
                        CtorKind::Fn,
                    ), _def_id), _) => {
                        err.note("a tuple struct constructor is private if any of its fields \
                                  is private");
                    }
                    NameBindingKind::Res(Res::Def(DefKind::Ctor(
                        CtorOf::Variant,
                        CtorKind::Fn,
                    ), _def_id), _) => {
                        err.note("a tuple variant constructor is private if any of its fields \
                                  is private");
                    }
                    _ => {}
                }
                err.emit();
            }
        }
    }

    fn report_with_use_injections(&mut self, krate: &Crate) {
        for UseError { mut err, candidates, node_id, better } in self.use_injections.drain(..) {
            let (span, found_use) = UsePlacementFinder::check(krate, node_id);
            if !candidates.is_empty() {
                diagnostics::show_candidates(&mut err, span, &candidates, better, found_use);
            }
            err.emit();
        }
    }

    fn report_conflict<'b>(&mut self,
                       parent: Module<'_>,
                       ident: Ident,
                       ns: Namespace,
                       new_binding: &NameBinding<'b>,
                       old_binding: &NameBinding<'b>) {
        // Error on the second of two conflicting names
        if old_binding.span.lo() > new_binding.span.lo() {
            return self.report_conflict(parent, ident, ns, old_binding, new_binding);
        }

        let container = match parent.kind {
            ModuleKind::Def(DefKind::Mod, _, _) => "module",
            ModuleKind::Def(DefKind::Trait, _, _) => "trait",
            ModuleKind::Block(..) => "block",
            _ => "enum",
        };

        let old_noun = match old_binding.is_import() {
            true => "import",
            false => "definition",
        };

        let new_participle = match new_binding.is_import() {
            true => "imported",
            false => "defined",
        };

        let (name, span) = (ident.name, self.session.source_map().def_span(new_binding.span));

        if let Some(s) = self.name_already_seen.get(&name) {
            if s == &span {
                return;
            }
        }

        let old_kind = match (ns, old_binding.module()) {
            (ValueNS, _) => "value",
            (MacroNS, _) => "macro",
            (TypeNS, _) if old_binding.is_extern_crate() => "extern crate",
            (TypeNS, Some(module)) if module.is_normal() => "module",
            (TypeNS, Some(module)) if module.is_trait() => "trait",
            (TypeNS, _) => "type",
        };

        let msg = format!("the name `{}` is defined multiple times", name);

        let mut err = match (old_binding.is_extern_crate(), new_binding.is_extern_crate()) {
            (true, true) => struct_span_err!(self.session, span, E0259, "{}", msg),
            (true, _) | (_, true) => match new_binding.is_import() && old_binding.is_import() {
                true => struct_span_err!(self.session, span, E0254, "{}", msg),
                false => struct_span_err!(self.session, span, E0260, "{}", msg),
            },
            _ => match (old_binding.is_import(), new_binding.is_import()) {
                (false, false) => struct_span_err!(self.session, span, E0428, "{}", msg),
                (true, true) => struct_span_err!(self.session, span, E0252, "{}", msg),
                _ => struct_span_err!(self.session, span, E0255, "{}", msg),
            },
        };

        err.note(&format!("`{}` must be defined only once in the {} namespace of this {}",
                          name,
                          ns.descr(),
                          container));

        err.span_label(span, format!("`{}` re{} here", name, new_participle));
        err.span_label(
            self.session.source_map().def_span(old_binding.span),
            format!("previous {} of the {} `{}` here", old_noun, old_kind, name),
        );

        // See https://github.com/rust-lang/rust/issues/32354
        use NameBindingKind::Import;
        let directive = match (&new_binding.kind, &old_binding.kind) {
            // If there are two imports where one or both have attributes then prefer removing the
            // import without attributes.
            (Import { directive: new, .. }, Import { directive: old, .. }) if {
                !new_binding.span.is_dummy() && !old_binding.span.is_dummy() &&
                    (new.has_attributes || old.has_attributes)
            } => {
                if old.has_attributes {
                    Some((new, new_binding.span, true))
                } else {
                    Some((old, old_binding.span, true))
                }
            },
            // Otherwise prioritize the new binding.
            (Import { directive, .. }, other) if !new_binding.span.is_dummy() =>
                Some((directive, new_binding.span, other.is_import())),
            (other, Import { directive, .. }) if !old_binding.span.is_dummy() =>
                Some((directive, old_binding.span, other.is_import())),
            _ => None,
        };

        // Check if the target of the use for both bindings is the same.
        let duplicate = new_binding.res().opt_def_id() == old_binding.res().opt_def_id();
        let has_dummy_span = new_binding.span.is_dummy() || old_binding.span.is_dummy();
        let from_item = self.extern_prelude.get(&ident)
            .map(|entry| entry.introduced_by_item)
            .unwrap_or(true);
        // Only suggest removing an import if both bindings are to the same def, if both spans
        // aren't dummy spans. Further, if both bindings are imports, then the ident must have
        // been introduced by a item.
        let should_remove_import = duplicate && !has_dummy_span &&
            ((new_binding.is_extern_crate() || old_binding.is_extern_crate()) || from_item);

        match directive {
            Some((directive, span, true)) if should_remove_import && directive.is_nested() =>
                self.add_suggestion_for_duplicate_nested_use(&mut err, directive, span),
            Some((directive, _, true)) if should_remove_import && !directive.is_glob() => {
                // Simple case - remove the entire import. Due to the above match arm, this can
                // only be a single use so just remove it entirely.
                err.tool_only_span_suggestion(
                    directive.use_span_with_attributes,
                    "remove unnecessary import",
                    String::new(),
                    Applicability::MaybeIncorrect,
                );
            },
            Some((directive, span, _)) =>
                self.add_suggestion_for_rename_of_use(&mut err, name, directive, span),
            _ => {},
        }

        err.emit();
        self.name_already_seen.insert(name, span);
    }

    /// This function adds a suggestion to change the binding name of a new import that conflicts
    /// with an existing import.
    ///
    /// ```ignore (diagnostic)
    /// help: you can use `as` to change the binding name of the import
    ///    |
    /// LL | use foo::bar as other_bar;
    ///    |     ^^^^^^^^^^^^^^^^^^^^^
    /// ```
    fn add_suggestion_for_rename_of_use(
        &self,
        err: &mut DiagnosticBuilder<'_>,
        name: Symbol,
        directive: &ImportDirective<'_>,
        binding_span: Span,
    ) {
        let suggested_name = if name.as_str().chars().next().unwrap().is_uppercase() {
            format!("Other{}", name)
        } else {
            format!("other_{}", name)
        };

        let mut suggestion = None;
        match directive.subclass {
            ImportDirectiveSubclass::SingleImport { type_ns_only: true, .. } =>
                suggestion = Some(format!("self as {}", suggested_name)),
            ImportDirectiveSubclass::SingleImport { source, .. } => {
                if let Some(pos) = source.span.hi().0.checked_sub(binding_span.lo().0)
                                                     .map(|pos| pos as usize) {
                    if let Ok(snippet) = self.session.source_map()
                                                     .span_to_snippet(binding_span) {
                        if pos <= snippet.len() {
                            suggestion = Some(format!(
                                "{} as {}{}",
                                &snippet[..pos],
                                suggested_name,
                                if snippet.ends_with(";") { ";" } else { "" }
                            ))
                        }
                    }
                }
            }
            ImportDirectiveSubclass::ExternCrate { source, target, .. } =>
                suggestion = Some(format!(
                    "extern crate {} as {};",
                    source.unwrap_or(target.name),
                    suggested_name,
                )),
            _ => unreachable!(),
        }

        let rename_msg = "you can use `as` to change the binding name of the import";
        if let Some(suggestion) = suggestion {
            err.span_suggestion(
                binding_span,
                rename_msg,
                suggestion,
                Applicability::MaybeIncorrect,
            );
        } else {
            err.span_label(binding_span, rename_msg);
        }
    }

    /// This function adds a suggestion to remove a unnecessary binding from an import that is
    /// nested. In the following example, this function will be invoked to remove the `a` binding
    /// in the second use statement:
    ///
    /// ```ignore (diagnostic)
    /// use issue_52891::a;
    /// use issue_52891::{d, a, e};
    /// ```
    ///
    /// The following suggestion will be added:
    ///
    /// ```ignore (diagnostic)
    /// use issue_52891::{d, a, e};
    ///                      ^-- help: remove unnecessary import
    /// ```
    ///
    /// If the nested use contains only one import then the suggestion will remove the entire
    /// line.
    ///
    /// It is expected that the directive provided is a nested import - this isn't checked by the
    /// function. If this invariant is not upheld, this function's behaviour will be unexpected
    /// as characters expected by span manipulations won't be present.
    fn add_suggestion_for_duplicate_nested_use(
        &self,
        err: &mut DiagnosticBuilder<'_>,
        directive: &ImportDirective<'_>,
        binding_span: Span,
    ) {
        assert!(directive.is_nested());
        let message = "remove unnecessary import";

        // Two examples will be used to illustrate the span manipulations we're doing:
        //
        // - Given `use issue_52891::{d, a, e};` where `a` is a duplicate then `binding_span` is
        //   `a` and `directive.use_span` is `issue_52891::{d, a, e};`.
        // - Given `use issue_52891::{d, e, a};` where `a` is a duplicate then `binding_span` is
        //   `a` and `directive.use_span` is `issue_52891::{d, e, a};`.

        let (found_closing_brace, span) = find_span_of_binding_until_next_binding(
            self.session, binding_span, directive.use_span,
        );

        // If there was a closing brace then identify the span to remove any trailing commas from
        // previous imports.
        if found_closing_brace {
            if let Some(span) = extend_span_to_previous_binding(self.session, span) {
                err.tool_only_span_suggestion(span, message, String::new(),
                                              Applicability::MaybeIncorrect);
            } else {
                // Remove the entire line if we cannot extend the span back, this indicates a
                // `issue_52891::{self}` case.
                err.span_suggestion(directive.use_span_with_attributes, message, String::new(),
                                    Applicability::MaybeIncorrect);
            }

            return;
        }

        err.span_suggestion(span, message, String::new(), Applicability::MachineApplicable);
    }

    fn extern_prelude_get(&mut self, ident: Ident, speculative: bool)
                          -> Option<&'a NameBinding<'a>> {
        if ident.is_path_segment_keyword() {
            // Make sure `self`, `super` etc produce an error when passed to here.
            return None;
        }
        self.extern_prelude.get(&ident.modern()).cloned().and_then(|entry| {
            if let Some(binding) = entry.extern_crate_item {
                if !speculative && entry.introduced_by_item {
                    self.record_use(ident, TypeNS, binding, false);
                }
                Some(binding)
            } else {
                let crate_id = if !speculative {
                    self.crate_loader.process_path_extern(ident.name, ident.span)
                } else if let Some(crate_id) =
                        self.crate_loader.maybe_process_path_extern(ident.name, ident.span) {
                    crate_id
                } else {
                    return None;
                };
                let crate_root = self.get_module(DefId { krate: crate_id, index: CRATE_DEF_INDEX });
                self.populate_module_if_necessary(&crate_root);
                Some((crate_root, ty::Visibility::Public, DUMMY_SP, ExpnId::root())
                    .to_name_binding(self.arenas))
            }
        })
    }

    /// Rustdoc uses this to resolve things in a recoverable way. `ResolutionError<'a>`
    /// isn't something that can be returned because it can't be made to live that long,
    /// and also it's a private type. Fortunately rustdoc doesn't need to know the error,
    /// just that an error occurred.
    pub fn resolve_str_path_error(&mut self, span: Span, path_str: &str, is_value: bool)
        -> Result<(ast::Path, Res), ()> {
        let path = if path_str.starts_with("::") {
            ast::Path {
                span,
                segments: iter::once(Ident::with_empty_ctxt(kw::PathRoot))
                    .chain({
                        path_str.split("::").skip(1).map(Ident::from_str)
                    })
                    .map(|i| self.new_ast_path_segment(i))
                    .collect(),
            }
        } else {
            ast::Path {
                span,
                segments: path_str
                    .split("::")
                    .map(Ident::from_str)
                    .map(|i| self.new_ast_path_segment(i))
                    .collect(),
            }
        };
        let res = self.resolve_ast_path_inner(&path, is_value).map_err(|_| ())?;
        Ok((path, res))
    }

    /// Like `resolve_ast_path`, but takes a callback in case there was an error.
    fn resolve_ast_path_inner(
        &mut self,
        path: &ast::Path,
        is_value: bool,
    ) -> Result<Res, (Span, ResolutionError<'a>)> {
        let namespace = if is_value { ValueNS } else { TypeNS };
        let span = path.span;
        let path = Segment::from_path(&path);
        // FIXME(Manishearth): intra-doc links won't get warned of epoch changes.
        let parent_scope = &self.dummy_parent_scope();
        match self.resolve_path(&path, Some(namespace), parent_scope, true, span, CrateLint::No) {
            PathResult::Module(ModuleOrUniformRoot::Module(module)) =>
                Ok(module.res().unwrap()),
            PathResult::NonModule(path_res) if path_res.unresolved_segments() == 0 =>
                Ok(path_res.base_res()),
            PathResult::NonModule(..) => {
                Err((span, ResolutionError::FailedToResolve {
                    label: String::from("type-relative paths are not supported in this context"),
                    suggestion: None,
                }))
            }
            PathResult::Module(..) | PathResult::Indeterminate => unreachable!(),
            PathResult::Failed { span, label, suggestion, .. } => {
                Err((span, ResolutionError::FailedToResolve {
                    label,
                    suggestion,
                }))
            }
        }
    }

    fn new_ast_path_segment(&self, ident: Ident) -> ast::PathSegment {
        let mut seg = ast::PathSegment::from_ident(ident);
        seg.id = self.session.next_node_id();
        seg
    }
}

fn names_to_string(idents: &[Ident]) -> String {
    let mut result = String::new();
    for (i, ident) in idents.iter()
                            .filter(|ident| ident.name != kw::PathRoot)
                            .enumerate() {
        if i > 0 {
            result.push_str("::");
        }
        result.push_str(&ident.as_str());
    }
    result
}

fn path_names_to_string(path: &Path) -> String {
    names_to_string(&path.segments.iter()
                        .map(|seg| seg.ident)
                        .collect::<Vec<_>>())
}

/// A somewhat inefficient routine to obtain the name of a module.
fn module_to_string(module: Module<'_>) -> Option<String> {
    let mut names = Vec::new();

    fn collect_mod(names: &mut Vec<Ident>, module: Module<'_>) {
        if let ModuleKind::Def(.., name) = module.kind {
            if let Some(parent) = module.parent {
                names.push(Ident::with_empty_ctxt(name));
                collect_mod(names, parent);
            }
        } else {
            // danger, shouldn't be ident?
            names.push(Ident::from_str("<opaque>"));
            collect_mod(names, module.parent.unwrap());
        }
    }
    collect_mod(&mut names, module);

    if names.is_empty() {
        return None;
    }
    Some(names_to_string(&names.into_iter()
                        .rev()
                        .collect::<Vec<_>>()))
}

#[derive(Copy, Clone, Debug)]
enum CrateLint {
    /// Do not issue the lint.
    No,

    /// This lint applies to some arbitrary path; e.g., `impl ::foo::Bar`.
    /// In this case, we can take the span of that path.
    SimplePath(NodeId),

    /// This lint comes from a `use` statement. In this case, what we
    /// care about really is the *root* `use` statement; e.g., if we
    /// have nested things like `use a::{b, c}`, we care about the
    /// `use a` part.
    UsePath { root_id: NodeId, root_span: Span },

    /// This is the "trait item" from a fully qualified path. For example,
    /// we might be resolving  `X::Y::Z` from a path like `<T as X::Y>::Z`.
    /// The `path_span` is the span of the to the trait itself (`X::Y`).
    QPathTrait { qpath_id: NodeId, qpath_span: Span },
}

impl CrateLint {
    fn node_id(&self) -> Option<NodeId> {
        match *self {
            CrateLint::No => None,
            CrateLint::SimplePath(id) |
            CrateLint::UsePath { root_id: id, .. } |
            CrateLint::QPathTrait { qpath_id: id, .. } => Some(id),
        }
    }
}

__build_diagnostic_array! { librustc_resolve, DIAGNOSTICS }
