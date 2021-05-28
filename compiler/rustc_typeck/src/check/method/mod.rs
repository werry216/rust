//! Method lookup: the secret sauce of Rust. See the [rustc dev guide] for more information.
//!
//! [rustc dev guide]: https://rustc-dev-guide.rust-lang.org/method-lookup.html

mod confirm;
pub mod probe;
mod suggest;

pub use self::suggest::{SelfSource, TraitInfo};
pub use self::CandidateSource::*;
pub use self::MethodError::*;

use crate::check::FnCtxt;
use rustc_ast::ast::Mutability;
use rustc_data_structures::sync::Lrc;
use rustc_errors::{Applicability, DiagnosticBuilder};
use rustc_hir as hir;
use rustc_hir::def::{CtorOf, DefKind, Namespace};
use rustc_hir::def_id::DefId;
use rustc_infer::infer::{self, InferOk};
use rustc_middle::ty::subst::Subst;
use rustc_middle::ty::subst::{InternalSubsts, SubstsRef};
use rustc_middle::ty::GenericParamDefKind;
use rustc_middle::ty::{self, ToPolyTraitRef, ToPredicate, Ty, TypeFoldable, WithConstness};
use rustc_session::lint::builtin::FUTURE_PRELUDE_COLLISION;
use rustc_span::edition::Edition;
use rustc_span::symbol::{sym, Ident};
use rustc_span::Span;
use rustc_trait_selection::traits;
use rustc_trait_selection::traits::query::evaluate_obligation::InferCtxtExt;

use self::probe::{IsSuggestion, ProbeScope};

pub fn provide(providers: &mut ty::query::Providers) {
    suggest::provide(providers);
    probe::provide(providers);
}

#[derive(Clone, Copy, Debug)]
pub struct MethodCallee<'tcx> {
    /// Impl method ID, for inherent methods, or trait method ID, otherwise.
    pub def_id: DefId,
    pub substs: SubstsRef<'tcx>,

    /// Instantiated method signature, i.e., it has been
    /// substituted, normalized, and has had late-bound
    /// lifetimes replaced with inference variables.
    pub sig: ty::FnSig<'tcx>,
}

#[derive(Debug)]
pub enum MethodError<'tcx> {
    // Did not find an applicable method, but we did find various near-misses that may work.
    NoMatch(NoMatchData<'tcx>),

    // Multiple methods might apply.
    Ambiguity(Vec<CandidateSource>),

    // Found an applicable method, but it is not visible. The third argument contains a list of
    // not-in-scope traits which may work.
    PrivateMatch(DefKind, DefId, Vec<DefId>),

    // Found a `Self: Sized` bound where `Self` is a trait object, also the caller may have
    // forgotten to import a trait.
    IllegalSizedBound(Vec<DefId>, bool, Span),

    // Found a match, but the return type is wrong
    BadReturnType,
}

// Contains a list of static methods that may apply, a list of unsatisfied trait predicates which
// could lead to matches if satisfied, and a list of not-in-scope traits which may work.
#[derive(Debug)]
pub struct NoMatchData<'tcx> {
    pub static_candidates: Vec<CandidateSource>,
    pub unsatisfied_predicates: Vec<(ty::Predicate<'tcx>, Option<ty::Predicate<'tcx>>)>,
    pub out_of_scope_traits: Vec<DefId>,
    pub lev_candidate: Option<ty::AssocItem>,
    pub mode: probe::Mode,
}

impl<'tcx> NoMatchData<'tcx> {
    pub fn new(
        static_candidates: Vec<CandidateSource>,
        unsatisfied_predicates: Vec<(ty::Predicate<'tcx>, Option<ty::Predicate<'tcx>>)>,
        out_of_scope_traits: Vec<DefId>,
        lev_candidate: Option<ty::AssocItem>,
        mode: probe::Mode,
    ) -> Self {
        NoMatchData {
            static_candidates,
            unsatisfied_predicates,
            out_of_scope_traits,
            lev_candidate,
            mode,
        }
    }
}

// A pared down enum describing just the places from which a method
// candidate can arise. Used for error reporting only.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CandidateSource {
    ImplSource(DefId),
    TraitSource(DefId /* trait id */),
}

impl<'a, 'tcx> FnCtxt<'a, 'tcx> {
    /// Determines whether the type `self_ty` supports a method name `method_name` or not.
    #[instrument(level = "debug", skip(self))]
    pub fn method_exists(
        &self,
        method_name: Ident,
        self_ty: Ty<'tcx>,
        call_expr_id: hir::HirId,
        allow_private: bool,
    ) -> bool {
        let mode = probe::Mode::MethodCall;
        match self.probe_for_name(
            method_name.span,
            mode,
            method_name,
            IsSuggestion(false),
            self_ty,
            call_expr_id,
            ProbeScope::TraitsInScope,
        ) {
            Ok(..) => true,
            Err(NoMatch(..)) => false,
            Err(Ambiguity(..)) => true,
            Err(PrivateMatch(..)) => allow_private,
            Err(IllegalSizedBound(..)) => true,
            Err(BadReturnType) => bug!("no return type expectations but got BadReturnType"),
        }
    }

    /// Adds a suggestion to call the given method to the provided diagnostic.
    #[instrument(level = "debug", skip(self, err, call_expr))]
    crate fn suggest_method_call(
        &self,
        err: &mut DiagnosticBuilder<'a>,
        msg: &str,
        method_name: Ident,
        self_ty: Ty<'tcx>,
        call_expr: &hir::Expr<'_>,
    ) {
        let params = self
            .probe_for_name(
                method_name.span,
                probe::Mode::MethodCall,
                method_name,
                IsSuggestion(false),
                self_ty,
                call_expr.hir_id,
                ProbeScope::TraitsInScope,
            )
            .map(|pick| {
                let sig = self.tcx.fn_sig(pick.item.def_id);
                sig.inputs().skip_binder().len().saturating_sub(1)
            })
            .unwrap_or(0);

        // Account for `foo.bar<T>`;
        let sugg_span = call_expr.span.shrink_to_hi();
        let (suggestion, applicability) = (
            format!("({})", (0..params).map(|_| "_").collect::<Vec<_>>().join(", ")),
            if params > 0 { Applicability::HasPlaceholders } else { Applicability::MaybeIncorrect },
        );

        err.span_suggestion_verbose(sugg_span, msg, suggestion, applicability);
    }

    /// Performs method lookup. If lookup is successful, it will return the callee
    /// and store an appropriate adjustment for the self-expr. In some cases it may
    /// report an error (e.g., invoking the `drop` method).
    ///
    /// # Arguments
    ///
    /// Given a method call like `foo.bar::<T1,...Tn>(...)`:
    ///
    /// * `self`:                  the surrounding `FnCtxt` (!)
    /// * `self_ty`:               the (unadjusted) type of the self expression (`foo`)
    /// * `segment`:               the name and generic arguments of the method (`bar::<T1, ...Tn>`)
    /// * `span`:                  the span for the method call
    /// * `call_expr`:             the complete method call: (`foo.bar::<T1,...Tn>(...)`)
    /// * `self_expr`:             the self expression (`foo`)
    #[instrument(level = "debug", skip(self, call_expr, self_expr))]
    pub fn lookup_method(
        &self,
        self_ty: Ty<'tcx>,
        segment: &hir::PathSegment<'_>,
        span: Span,
        call_expr: &'tcx hir::Expr<'tcx>,
        self_expr: &'tcx hir::Expr<'tcx>,
    ) -> Result<MethodCallee<'tcx>, MethodError<'tcx>> {
        debug!(
            "lookup(method_name={}, self_ty={:?}, call_expr={:?}, self_expr={:?})",
            segment.ident, self_ty, call_expr, self_expr
        );

        let pick =
            self.lookup_probe(span, segment.ident, self_ty, call_expr, ProbeScope::TraitsInScope)?;

        if span.edition() < Edition::Edition2021 {
            if let sym::try_into = segment.ident.name {
                if !matches!(self.tcx.crate_name(pick.item.def_id.krate), sym::std | sym::core) {
                    self.tcx.struct_span_lint_hir(
                        FUTURE_PRELUDE_COLLISION,
                        call_expr.hir_id,
                        call_expr.span,
                        |lint| {
                            let sp = call_expr.span;
                            let type_name = self.tcx.def_path_str(pick.item.container.id());
                            let type_generics = self.tcx.generics_of(pick.item.container.id());
                            let parameter_count =
                                type_generics.count() - (type_generics.has_self as usize);
                            let trait_name = if parameter_count == 0 {
                                type_name
                            } else {
                                format!(
                                    "{}<{}>",
                                    type_name,
                                    std::iter::repeat("_")
                                        .take(parameter_count)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            };

                            let mut lint = lint.build(&format!(
                                "trait method `{}` will become ambiguous in Rust 2021",
                                segment.ident.name
                            ));

                            if let Ok(self_expr) =
                                self.sess().source_map().span_to_snippet(self_expr.span)
                            {
                                let derefs = "*".repeat(pick.autoderefs);

                                let autoref = match pick.autoref_or_ptr_adjustment {
                                    Some(probe::AutorefOrPtrAdjustment::Autoref {
                                        mutbl: Mutability::Mut,
                                        ..
                                    }) => "&mut ",
                                    Some(probe::AutorefOrPtrAdjustment::Autoref {
                                        mutbl: Mutability::Not,
                                        ..
                                    }) => "&",
                                    Some(probe::AutorefOrPtrAdjustment::ToConstPtr) | None => "",
                                };
                                let self_adjusted =
                                    if let Some(probe::AutorefOrPtrAdjustment::ToConstPtr) =
                                        pick.autoref_or_ptr_adjustment
                                    {
                                        format!("{}{} as *const _", derefs, self_expr)
                                    } else {
                                        format!("{}{}{}", autoref, derefs, self_expr)
                                    };
                                lint.span_suggestion(
                                    sp,
                                    "disambiguate the associated function",
                                    format!(
                                        "{}::{}({})",
                                        trait_name, segment.ident.name, self_adjusted,
                                    ),
                                    Applicability::MachineApplicable,
                                );
                            } else {
                                lint.span_help(
                                    sp,
                                    &format!(
                                        "disambiguate the associated function with `{}::{}(...)`",
                                        trait_name, segment.ident,
                                    ),
                                );
                            }

                            lint.emit();
                        },
                    );
                }
            }
        }

        for import_id in &pick.import_ids {
            debug!("used_trait_import: {:?}", import_id);
            Lrc::get_mut(&mut self.typeck_results.borrow_mut().used_trait_imports)
                .unwrap()
                .insert(*import_id);
        }

        self.tcx.check_stability(pick.item.def_id, Some(call_expr.hir_id), span, None);

        let result =
            self.confirm_method(span, self_expr, call_expr, self_ty, pick.clone(), segment);
        debug!("result = {:?}", result);

        if let Some(span) = result.illegal_sized_bound {
            let mut needs_mut = false;
            if let ty::Ref(region, t_type, mutability) = self_ty.kind() {
                let trait_type = self
                    .tcx
                    .mk_ref(region, ty::TypeAndMut { ty: t_type, mutbl: mutability.invert() });
                // We probe again to see if there might be a borrow mutability discrepancy.
                match self.lookup_probe(
                    span,
                    segment.ident,
                    trait_type,
                    call_expr,
                    ProbeScope::TraitsInScope,
                ) {
                    Ok(ref new_pick) if *new_pick != pick => {
                        needs_mut = true;
                    }
                    _ => {}
                }
            }

            // We probe again, taking all traits into account (not only those in scope).
            let candidates = match self.lookup_probe(
                span,
                segment.ident,
                self_ty,
                call_expr,
                ProbeScope::AllTraits,
            ) {
                // If we find a different result the caller probably forgot to import a trait.
                Ok(ref new_pick) if *new_pick != pick => vec![new_pick.item.container.id()],
                Err(Ambiguity(ref sources)) => sources
                    .iter()
                    .filter_map(|source| {
                        match *source {
                            // Note: this cannot come from an inherent impl,
                            // because the first probing succeeded.
                            ImplSource(def) => self.tcx.trait_id_of_impl(def),
                            TraitSource(_) => None,
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };

            return Err(IllegalSizedBound(candidates, needs_mut, span));
        }

        Ok(result.callee)
    }

    #[instrument(level = "debug", skip(self, call_expr))]
    pub fn lookup_probe(
        &self,
        span: Span,
        method_name: Ident,
        self_ty: Ty<'tcx>,
        call_expr: &'tcx hir::Expr<'tcx>,
        scope: ProbeScope,
    ) -> probe::PickResult<'tcx> {
        let mode = probe::Mode::MethodCall;
        let self_ty = self.resolve_vars_if_possible(self_ty);
        self.probe_for_name(
            span,
            mode,
            method_name,
            IsSuggestion(false),
            self_ty,
            call_expr.hir_id,
            scope,
        )
    }

    /// `lookup_method_in_trait` is used for overloaded operators.
    /// It does a very narrow slice of what the normal probe/confirm path does.
    /// In particular, it doesn't really do any probing: it simply constructs
    /// an obligation for a particular trait with the given self type and checks
    /// whether that trait is implemented.
    //
    // FIXME(#18741): it seems likely that we can consolidate some of this
    // code with the other method-lookup code. In particular, the second half
    // of this method is basically the same as confirmation.
    #[instrument(level = "debug", skip(self, span, opt_input_types))]
    pub fn lookup_method_in_trait(
        &self,
        span: Span,
        m_name: Ident,
        trait_def_id: DefId,
        self_ty: Ty<'tcx>,
        opt_input_types: Option<&[Ty<'tcx>]>,
    ) -> Option<InferOk<'tcx, MethodCallee<'tcx>>> {
        debug!(
            "lookup_in_trait_adjusted(self_ty={:?}, m_name={}, trait_def_id={:?}, opt_input_types={:?})",
            self_ty, m_name, trait_def_id, opt_input_types
        );

        // Construct a trait-reference `self_ty : Trait<input_tys>`
        let substs = InternalSubsts::for_item(self.tcx, trait_def_id, |param, _| {
            match param.kind {
                GenericParamDefKind::Lifetime | GenericParamDefKind::Const { .. } => {}
                GenericParamDefKind::Type { .. } => {
                    if param.index == 0 {
                        return self_ty.into();
                    } else if let Some(ref input_types) = opt_input_types {
                        return input_types[param.index as usize - 1].into();
                    }
                }
            }
            self.var_for_def(span, param)
        });

        let trait_ref = ty::TraitRef::new(trait_def_id, substs);

        // Construct an obligation
        let poly_trait_ref = trait_ref.to_poly_trait_ref();
        let obligation = traits::Obligation::misc(
            span,
            self.body_id,
            self.param_env,
            poly_trait_ref.without_const().to_predicate(self.tcx),
        );

        // Now we want to know if this can be matched
        if !self.predicate_may_hold(&obligation) {
            debug!("--> Cannot match obligation");
            return None; // Cannot be matched, no such method resolution is possible.
        }

        // Trait must have a method named `m_name` and it should not have
        // type parameters or early-bound regions.
        let tcx = self.tcx;
        let method_item = match self.associated_item(trait_def_id, m_name, Namespace::ValueNS) {
            Some(method_item) => method_item,
            None => {
                tcx.sess.delay_span_bug(
                    span,
                    "operator trait does not have corresponding operator method",
                );
                return None;
            }
        };
        let def_id = method_item.def_id;
        let generics = tcx.generics_of(def_id);
        assert_eq!(generics.params.len(), 0);

        debug!("lookup_in_trait_adjusted: method_item={:?}", method_item);
        let mut obligations = vec![];

        // Instantiate late-bound regions and substitute the trait
        // parameters into the method type to get the actual method type.
        //
        // N.B., instantiate late-bound regions first so that
        // `instantiate_type_scheme` can normalize associated types that
        // may reference those regions.
        let fn_sig = tcx.fn_sig(def_id);
        let fn_sig = self.replace_bound_vars_with_fresh_vars(span, infer::FnCall, fn_sig).0;
        let fn_sig = fn_sig.subst(self.tcx, substs);

        let InferOk { value, obligations: o } =
            self.normalize_associated_types_in_as_infer_ok(span, fn_sig);
        let fn_sig = {
            obligations.extend(o);
            value
        };

        // Register obligations for the parameters. This will include the
        // `Self` parameter, which in turn has a bound of the main trait,
        // so this also effectively registers `obligation` as well.  (We
        // used to register `obligation` explicitly, but that resulted in
        // double error messages being reported.)
        //
        // Note that as the method comes from a trait, it should not have
        // any late-bound regions appearing in its bounds.
        let bounds = self.tcx.predicates_of(def_id).instantiate(self.tcx, substs);

        let InferOk { value, obligations: o } =
            self.normalize_associated_types_in_as_infer_ok(span, bounds);
        let bounds = {
            obligations.extend(o);
            value
        };

        assert!(!bounds.has_escaping_bound_vars());

        let cause = traits::ObligationCause::misc(span, self.body_id);
        obligations.extend(traits::predicates_for_generics(cause.clone(), self.param_env, bounds));

        // Also add an obligation for the method type being well-formed.
        let method_ty = tcx.mk_fn_ptr(ty::Binder::bind(fn_sig, tcx));
        debug!(
            "lookup_in_trait_adjusted: matched method method_ty={:?} obligation={:?}",
            method_ty, obligation
        );
        obligations.push(traits::Obligation::new(
            cause,
            self.param_env,
            ty::PredicateKind::WellFormed(method_ty.into()).to_predicate(tcx),
        ));

        let callee = MethodCallee { def_id, substs: trait_ref.substs, sig: fn_sig };

        debug!("callee = {:?}", callee);

        Some(InferOk { obligations, value: callee })
    }

    /// Performs "universal function call" lookup. If lookup is successful, it will return the type
    /// of definition and the [`DefId`] of the found function definition.
    ///
    /// # Arguments
    ///
    /// Given a function call like `Foo::bar::<T1,...Tn>(...)`:
    ///
    /// * `self`:                  the surrounding `FnCtxt` (!)
    /// * `span`:                  the span of the call, excluding arguments (`Foo::bar::<T1, ...Tn>`)
    /// * `method_name`:           the identifier of the function within the container type (`bar`)
    /// * `self_ty`:               the type to search within (`Foo`)
    /// * `expr_id`:               the [`hir::HirId`] of the expression composing the entire call
    #[instrument(level = "debug", skip(self))]
    pub fn resolve_ufcs(
        &self,
        span: Span,
        method_name: Ident,
        self_ty: Ty<'tcx>,
        expr_id: hir::HirId,
    ) -> Result<(DefKind, DefId), MethodError<'tcx>> {
        debug!(
            "resolve_ufcs: method_name={:?} self_ty={:?} expr_id={:?}",
            method_name, self_ty, expr_id,
        );

        let tcx = self.tcx;

        // Check if we have an enum variant.
        if let ty::Adt(adt_def, _) = self_ty.kind() {
            if adt_def.is_enum() {
                let variant_def = adt_def
                    .variants
                    .iter()
                    .find(|vd| tcx.hygienic_eq(method_name, vd.ident, adt_def.did));
                if let Some(variant_def) = variant_def {
                    // Braced variants generate unusable names in value namespace (reserved for
                    // possible future use), so variants resolved as associated items may refer to
                    // them as well. It's ok to use the variant's id as a ctor id since an
                    // error will be reported on any use of such resolution anyway.
                    let ctor_def_id = variant_def.ctor_def_id.unwrap_or(variant_def.def_id);
                    tcx.check_stability(ctor_def_id, Some(expr_id), span, Some(method_name.span));
                    return Ok((
                        DefKind::Ctor(CtorOf::Variant, variant_def.ctor_kind),
                        ctor_def_id,
                    ));
                }
            }
        }

        let pick = self.probe_for_name(
            span,
            probe::Mode::Path,
            method_name,
            IsSuggestion(false),
            self_ty,
            expr_id,
            ProbeScope::TraitsInScope,
        )?;

        if span.edition() < Edition::Edition2021 {
            if let sym::try_into | sym::try_from | sym::from_iter = method_name.name {
                if !matches!(tcx.crate_name(pick.item.def_id.krate), sym::std | sym::core) {
                    tcx.struct_span_lint_hir(FUTURE_PRELUDE_COLLISION, expr_id, span, |lint| {
                        // "type" refers to either a type or, more likely, a trait from which
                        // the associated function or method is from.
                        let type_name = tcx.def_path_str(pick.item.container.id());
                        let type_generics = tcx.generics_of(pick.item.container.id());

                        let parameter_count =
                            type_generics.count() - (type_generics.has_self as usize);
                        let trait_name = if parameter_count == 0 {
                            type_name
                        } else {
                            format!(
                                "{}<{}>",
                                type_name,
                                std::iter::repeat("_")
                                    .take(parameter_count)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        };

                        let mut lint = lint.build(&format!(
                            "trait-associated function `{}` will become ambiguous in Rust 2021",
                            method_name.name
                        ));

                        lint.span_suggestion(
                            span,
                            "disambiguate the associated function",
                            format!("<{} as {}>::{}", self_ty, trait_name, method_name.name,),
                            Applicability::MachineApplicable,
                        );

                        lint.emit();
                    });
                }
            }
        }

        debug!("resolve_ufcs: pick={:?}", pick);
        {
            let mut typeck_results = self.typeck_results.borrow_mut();
            let used_trait_imports = Lrc::get_mut(&mut typeck_results.used_trait_imports).unwrap();
            for import_id in pick.import_ids {
                debug!("resolve_ufcs: used_trait_import: {:?}", import_id);
                used_trait_imports.insert(import_id);
            }
        }

        let def_kind = pick.item.kind.as_def_kind();
        debug!("resolve_ufcs: def_kind={:?}, def_id={:?}", def_kind, pick.item.def_id);
        tcx.check_stability(pick.item.def_id, Some(expr_id), span, Some(method_name.span));
        Ok((def_kind, pick.item.def_id))
    }

    /// Finds item with name `item_name` defined in impl/trait `def_id`
    /// and return it, or `None`, if no such item was defined there.
    pub fn associated_item(
        &self,
        def_id: DefId,
        item_name: Ident,
        ns: Namespace,
    ) -> Option<ty::AssocItem> {
        self.tcx
            .associated_items(def_id)
            .find_by_name_and_namespace(self.tcx, item_name, ns, def_id)
            .copied()
    }
}
