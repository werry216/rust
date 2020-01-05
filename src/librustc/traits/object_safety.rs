//! "Object safety" refers to the ability for a trait to be converted
//! to an object. In general, traits may only be converted to an
//! object if all of their methods meet certain criteria. In particular,
//! they must:
//!
//!   - have a suitable receiver from which we can extract a vtable and coerce to a "thin" version
//!     that doesn't contain the vtable;
//!   - not reference the erased type `Self` except for in this receiver;
//!   - not have generic type parameters.

use super::elaborate_predicates;

use crate::lint;
use crate::traits::{self, Obligation, ObligationCause};
use crate::ty::subst::{InternalSubsts, Subst};
use crate::ty::{self, Predicate, ToPredicate, Ty, TyCtxt, TypeFoldable};
use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_span::symbol::Symbol;
use rustc_span::{Span, DUMMY_SP};
use std::borrow::Cow;
use std::iter::{self};
use syntax::ast::{self};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectSafetyViolation {
    /// `Self: Sized` declared on the trait.
    SizedSelf,

    /// Supertrait reference references `Self` an in illegal location
    /// (e.g., `trait Foo : Bar<Self>`).
    SupertraitSelf,

    /// Method has something illegal.
    Method(ast::Name, MethodViolationCode, Span),

    /// Associated const.
    AssocConst(ast::Name, Span),
}

impl ObjectSafetyViolation {
    pub fn error_msg(&self) -> Cow<'static, str> {
        match *self {
            ObjectSafetyViolation::SizedSelf => {
                "the trait cannot require that `Self : Sized`".into()
            }
            ObjectSafetyViolation::SupertraitSelf => {
                "the trait cannot use `Self` as a type parameter \
                 in the supertraits or where-clauses"
                    .into()
            }
            ObjectSafetyViolation::Method(name, MethodViolationCode::StaticMethod, _) => {
                format!("associated function `{}` has no `self` parameter", name).into()
            }
            ObjectSafetyViolation::Method(name, MethodViolationCode::ReferencesSelf, _) => format!(
                "method `{}` references the `Self` type in its parameters or return type",
                name,
            )
            .into(),
            ObjectSafetyViolation::Method(
                name,
                MethodViolationCode::WhereClauseReferencesSelf,
                _,
            ) => format!("method `{}` references the `Self` type in where clauses", name).into(),
            ObjectSafetyViolation::Method(name, MethodViolationCode::Generic, _) => {
                format!("method `{}` has generic type parameters", name).into()
            }
            ObjectSafetyViolation::Method(name, MethodViolationCode::UndispatchableReceiver, _) => {
                format!("method `{}`'s `self` parameter cannot be dispatched on", name).into()
            }
            ObjectSafetyViolation::AssocConst(name, _) => {
                format!("the trait cannot contain associated consts like `{}`", name).into()
            }
        }
    }

    pub fn span(&self) -> Option<Span> {
        // When `span` comes from a separate crate, it'll be `DUMMY_SP`. Treat it as `None` so
        // diagnostics use a `note` instead of a `span_label`.
        match *self {
            ObjectSafetyViolation::AssocConst(_, span)
            | ObjectSafetyViolation::Method(_, _, span)
                if span != DUMMY_SP =>
            {
                Some(span)
            }
            _ => None,
        }
    }
}

/// Reasons a method might not be object-safe.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MethodViolationCode {
    /// e.g., `fn foo()`
    StaticMethod,

    /// e.g., `fn foo(&self, x: Self)` or `fn foo(&self) -> Self`
    ReferencesSelf,

    /// e.g., `fn foo(&self) where Self: Clone`
    WhereClauseReferencesSelf,

    /// e.g., `fn foo<A>()`
    Generic,

    /// the method's receiver (`self` argument) can't be dispatched on
    UndispatchableReceiver,
}

impl<'tcx> TyCtxt<'tcx> {
    /// Returns the object safety violations that affect
    /// astconv -- currently, `Self` in supertraits. This is needed
    /// because `object_safety_violations` can't be used during
    /// type collection.
    pub fn astconv_object_safety_violations(
        self,
        trait_def_id: DefId,
    ) -> Vec<ObjectSafetyViolation> {
        debug_assert!(self.generics_of(trait_def_id).has_self);
        let violations = traits::supertrait_def_ids(self, trait_def_id)
            .filter(|&def_id| predicates_reference_self(self, def_id, true))
            .map(|_| ObjectSafetyViolation::SupertraitSelf)
            .collect();

        debug!(
            "astconv_object_safety_violations(trait_def_id={:?}) = {:?}",
            trait_def_id, violations
        );

        violations
    }

    pub fn object_safety_violations(self, trait_def_id: DefId) -> Vec<ObjectSafetyViolation> {
        debug_assert!(self.generics_of(trait_def_id).has_self);
        debug!("object_safety_violations: {:?}", trait_def_id);

        traits::supertrait_def_ids(self, trait_def_id)
            .flat_map(|def_id| object_safety_violations_for_trait(self, def_id))
            .collect()
    }

    /// We say a method is *vtable safe* if it can be invoked on a trait
    /// object. Note that object-safe traits can have some
    /// non-vtable-safe methods, so long as they require `Self: Sized` or
    /// otherwise ensure that they cannot be used when `Self = Trait`.
    pub fn is_vtable_safe_method(self, trait_def_id: DefId, method: &ty::AssocItem) -> bool {
        debug_assert!(self.generics_of(trait_def_id).has_self);
        debug!("is_vtable_safe_method({:?}, {:?})", trait_def_id, method);
        // Any method that has a `Self: Sized` bound cannot be called.
        if generics_require_sized_self(self, method.def_id) {
            return false;
        }

        match virtual_call_violation_for_method(self, trait_def_id, method) {
            None | Some(MethodViolationCode::WhereClauseReferencesSelf) => true,
            Some(_) => false,
        }
    }
}

fn object_safety_violations_for_trait(
    tcx: TyCtxt<'_>,
    trait_def_id: DefId,
) -> Vec<ObjectSafetyViolation> {
    // Check methods for violations.
    let mut violations: Vec<_> = tcx
        .associated_items(trait_def_id)
        .filter(|item| item.kind == ty::AssocKind::Method)
        .filter_map(|item| {
            object_safety_violation_for_method(tcx, trait_def_id, &item)
                .map(|code| ObjectSafetyViolation::Method(item.ident.name, code, item.ident.span))
        })
        .filter(|violation| {
            if let ObjectSafetyViolation::Method(
                _,
                MethodViolationCode::WhereClauseReferencesSelf,
                span,
            ) = violation
            {
                // Using `CRATE_NODE_ID` is wrong, but it's hard to get a more precise id.
                // It's also hard to get a use site span, so we use the method definition span.
                tcx.lint_node_note(
                    lint::builtin::WHERE_CLAUSES_OBJECT_SAFETY,
                    hir::CRATE_HIR_ID,
                    *span,
                    &format!(
                        "the trait `{}` cannot be made into an object",
                        tcx.def_path_str(trait_def_id)
                    ),
                    &violation.error_msg(),
                );
                false
            } else {
                true
            }
        })
        .collect();

    // Check the trait itself.
    if trait_has_sized_self(tcx, trait_def_id) {
        violations.push(ObjectSafetyViolation::SizedSelf);
    }
    if predicates_reference_self(tcx, trait_def_id, false) {
        violations.push(ObjectSafetyViolation::SupertraitSelf);
    }

    violations.extend(
        tcx.associated_items(trait_def_id)
            .filter(|item| item.kind == ty::AssocKind::Const)
            .map(|item| ObjectSafetyViolation::AssocConst(item.ident.name, item.ident.span)),
    );

    debug!(
        "object_safety_violations_for_trait(trait_def_id={:?}) = {:?}",
        trait_def_id, violations
    );

    violations
}

fn predicates_reference_self(tcx: TyCtxt<'_>, trait_def_id: DefId, supertraits_only: bool) -> bool {
    let trait_ref = ty::Binder::dummy(ty::TraitRef::identity(tcx, trait_def_id));
    let predicates = if supertraits_only {
        tcx.super_predicates_of(trait_def_id)
    } else {
        tcx.predicates_of(trait_def_id)
    };
    let self_ty = tcx.types.self_param;
    let has_self_ty = |t: Ty<'_>| t.walk().any(|t| t == self_ty);
    predicates
        .predicates
        .iter()
        .map(|(predicate, _)| predicate.subst_supertrait(tcx, &trait_ref))
        .any(|predicate| {
            match predicate {
                ty::Predicate::Trait(ref data) => {
                    // In the case of a trait predicate, we can skip the "self" type.
                    data.skip_binder().input_types().skip(1).any(has_self_ty)
                }
                ty::Predicate::Projection(ref data) => {
                    // And similarly for projections. This should be redundant with
                    // the previous check because any projection should have a
                    // matching `Trait` predicate with the same inputs, but we do
                    // the check to be safe.
                    //
                    // Note that we *do* allow projection *outputs* to contain
                    // `self` (i.e., `trait Foo: Bar<Output=Self::Result> { type Result; }`),
                    // we just require the user to specify *both* outputs
                    // in the object type (i.e., `dyn Foo<Output=(), Result=()>`).
                    //
                    // This is ALT2 in issue #56288, see that for discussion of the
                    // possible alternatives.
                    data.skip_binder()
                        .projection_ty
                        .trait_ref(tcx)
                        .input_types()
                        .skip(1)
                        .any(has_self_ty)
                }
                ty::Predicate::WellFormed(..)
                | ty::Predicate::ObjectSafe(..)
                | ty::Predicate::TypeOutlives(..)
                | ty::Predicate::RegionOutlives(..)
                | ty::Predicate::ClosureKind(..)
                | ty::Predicate::Subtype(..)
                | ty::Predicate::ConstEvaluatable(..) => false,
            }
        })
}

fn trait_has_sized_self(tcx: TyCtxt<'_>, trait_def_id: DefId) -> bool {
    generics_require_sized_self(tcx, trait_def_id)
}

fn generics_require_sized_self(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    let sized_def_id = match tcx.lang_items().sized_trait() {
        Some(def_id) => def_id,
        None => {
            return false; /* No Sized trait, can't require it! */
        }
    };

    // Search for a predicate like `Self : Sized` amongst the trait bounds.
    let predicates = tcx.predicates_of(def_id);
    let predicates = predicates.instantiate_identity(tcx).predicates;
    elaborate_predicates(tcx, predicates).any(|predicate| match predicate {
        ty::Predicate::Trait(ref trait_pred) => {
            trait_pred.def_id() == sized_def_id && trait_pred.skip_binder().self_ty().is_param(0)
        }
        ty::Predicate::Projection(..)
        | ty::Predicate::Subtype(..)
        | ty::Predicate::RegionOutlives(..)
        | ty::Predicate::WellFormed(..)
        | ty::Predicate::ObjectSafe(..)
        | ty::Predicate::ClosureKind(..)
        | ty::Predicate::TypeOutlives(..)
        | ty::Predicate::ConstEvaluatable(..) => false,
    })
}

/// Returns `Some(_)` if this method makes the containing trait not object safe.
fn object_safety_violation_for_method(
    tcx: TyCtxt<'_>,
    trait_def_id: DefId,
    method: &ty::AssocItem,
) -> Option<MethodViolationCode> {
    debug!("object_safety_violation_for_method({:?}, {:?})", trait_def_id, method);
    // Any method that has a `Self : Sized` requisite is otherwise
    // exempt from the regulations.
    if generics_require_sized_self(tcx, method.def_id) {
        return None;
    }

    virtual_call_violation_for_method(tcx, trait_def_id, method)
}

/// Returns `Some(_)` if this method cannot be called on a trait
/// object; this does not necessarily imply that the enclosing trait
/// is not object safe, because the method might have a where clause
/// `Self:Sized`.
fn virtual_call_violation_for_method<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_def_id: DefId,
    method: &ty::AssocItem,
) -> Option<MethodViolationCode> {
    // The method's first parameter must be named `self`
    if !method.method_has_self_argument {
        return Some(MethodViolationCode::StaticMethod);
    }

    let sig = tcx.fn_sig(method.def_id);

    for input_ty in &sig.skip_binder().inputs()[1..] {
        if contains_illegal_self_type_reference(tcx, trait_def_id, input_ty) {
            return Some(MethodViolationCode::ReferencesSelf);
        }
    }
    if contains_illegal_self_type_reference(tcx, trait_def_id, sig.output().skip_binder()) {
        return Some(MethodViolationCode::ReferencesSelf);
    }

    // We can't monomorphize things like `fn foo<A>(...)`.
    let own_counts = tcx.generics_of(method.def_id).own_counts();
    if own_counts.types + own_counts.consts != 0 {
        return Some(MethodViolationCode::Generic);
    }

    if tcx
        .predicates_of(method.def_id)
        .predicates
        .iter()
        // A trait object can't claim to live more than the concrete type,
        // so outlives predicates will always hold.
        .cloned()
        .filter(|(p, _)| p.to_opt_type_outlives().is_none())
        .collect::<Vec<_>>()
        // Do a shallow visit so that `contains_illegal_self_type_reference`
        // may apply it's custom visiting.
        .visit_tys_shallow(|t| contains_illegal_self_type_reference(tcx, trait_def_id, t))
    {
        return Some(MethodViolationCode::WhereClauseReferencesSelf);
    }

    let receiver_ty =
        tcx.liberate_late_bound_regions(method.def_id, &sig.map_bound(|sig| sig.inputs()[0]));

    // Until `unsized_locals` is fully implemented, `self: Self` can't be dispatched on.
    // However, this is already considered object-safe. We allow it as a special case here.
    // FIXME(mikeyhew) get rid of this `if` statement once `receiver_is_dispatchable` allows
    // `Receiver: Unsize<Receiver[Self => dyn Trait]>`.
    if receiver_ty != tcx.types.self_param {
        if !receiver_is_dispatchable(tcx, method, receiver_ty) {
            return Some(MethodViolationCode::UndispatchableReceiver);
        } else {
            // Do sanity check to make sure the receiver actually has the layout of a pointer.

            use crate::ty::layout::Abi;

            let param_env = tcx.param_env(method.def_id);

            let abi_of_ty = |ty: Ty<'tcx>| -> &Abi {
                match tcx.layout_of(param_env.and(ty)) {
                    Ok(layout) => &layout.abi,
                    Err(err) => bug!("error: {}\n while computing layout for type {:?}", err, ty),
                }
            };

            // e.g., `Rc<()>`
            let unit_receiver_ty =
                receiver_for_self_ty(tcx, receiver_ty, tcx.mk_unit(), method.def_id);

            match abi_of_ty(unit_receiver_ty) {
                &Abi::Scalar(..) => (),
                abi => {
                    tcx.sess.delay_span_bug(
                        tcx.def_span(method.def_id),
                        &format!(
                            "receiver when `Self = ()` should have a Scalar ABI; found {:?}",
                            abi
                        ),
                    );
                }
            }

            let trait_object_ty =
                object_ty_for_trait(tcx, trait_def_id, tcx.mk_region(ty::ReStatic));

            // e.g., `Rc<dyn Trait>`
            let trait_object_receiver =
                receiver_for_self_ty(tcx, receiver_ty, trait_object_ty, method.def_id);

            match abi_of_ty(trait_object_receiver) {
                &Abi::ScalarPair(..) => (),
                abi => {
                    tcx.sess.delay_span_bug(
                        tcx.def_span(method.def_id),
                        &format!(
                            "receiver when `Self = {}` should have a ScalarPair ABI; \
                                 found {:?}",
                            trait_object_ty, abi
                        ),
                    );
                }
            }
        }
    }

    None
}

/// Performs a type substitution to produce the version of `receiver_ty` when `Self = self_ty`.
/// For example, for `receiver_ty = Rc<Self>` and `self_ty = Foo`, returns `Rc<Foo>`.
fn receiver_for_self_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    receiver_ty: Ty<'tcx>,
    self_ty: Ty<'tcx>,
    method_def_id: DefId,
) -> Ty<'tcx> {
    debug!("receiver_for_self_ty({:?}, {:?}, {:?})", receiver_ty, self_ty, method_def_id);
    let substs = InternalSubsts::for_item(tcx, method_def_id, |param, _| {
        if param.index == 0 { self_ty.into() } else { tcx.mk_param_from_def(param) }
    });

    let result = receiver_ty.subst(tcx, substs);
    debug!(
        "receiver_for_self_ty({:?}, {:?}, {:?}) = {:?}",
        receiver_ty, self_ty, method_def_id, result
    );
    result
}

/// Creates the object type for the current trait. For example,
/// if the current trait is `Deref`, then this will be
/// `dyn Deref<Target = Self::Target> + 'static`.
fn object_ty_for_trait<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_def_id: DefId,
    lifetime: ty::Region<'tcx>,
) -> Ty<'tcx> {
    debug!("object_ty_for_trait: trait_def_id={:?}", trait_def_id);

    let trait_ref = ty::TraitRef::identity(tcx, trait_def_id);

    let trait_predicate =
        ty::ExistentialPredicate::Trait(ty::ExistentialTraitRef::erase_self_ty(tcx, trait_ref));

    let mut associated_types = traits::supertraits(tcx, ty::Binder::dummy(trait_ref))
        .flat_map(|super_trait_ref| {
            tcx.associated_items(super_trait_ref.def_id()).map(move |item| (super_trait_ref, item))
        })
        .filter(|(_, item)| item.kind == ty::AssocKind::Type)
        .collect::<Vec<_>>();

    // existential predicates need to be in a specific order
    associated_types.sort_by_cached_key(|(_, item)| tcx.def_path_hash(item.def_id));

    let projection_predicates = associated_types.into_iter().map(|(super_trait_ref, item)| {
        // We *can* get bound lifetimes here in cases like
        // `trait MyTrait: for<'s> OtherTrait<&'s T, Output=bool>`.
        //
        // binder moved to (*)...
        let super_trait_ref = super_trait_ref.skip_binder();
        ty::ExistentialPredicate::Projection(ty::ExistentialProjection {
            ty: tcx.mk_projection(item.def_id, super_trait_ref.substs),
            item_def_id: item.def_id,
            substs: super_trait_ref.substs,
        })
    });

    let existential_predicates =
        tcx.mk_existential_predicates(iter::once(trait_predicate).chain(projection_predicates));

    let object_ty = tcx.mk_dynamic(
        // (*) ... binder re-introduced here
        ty::Binder::bind(existential_predicates),
        lifetime,
    );

    debug!("object_ty_for_trait: object_ty=`{}`", object_ty);

    object_ty
}

/// Checks the method's receiver (the `self` argument) can be dispatched on when `Self` is a
/// trait object. We require that `DispatchableFromDyn` be implemented for the receiver type
/// in the following way:
/// - let `Receiver` be the type of the `self` argument, i.e `Self`, `&Self`, `Rc<Self>`,
/// - require the following bound:
///
///   ```
///   Receiver[Self => T]: DispatchFromDyn<Receiver[Self => dyn Trait]>
///   ```
///
///   where `Foo[X => Y]` means "the same type as `Foo`, but with `X` replaced with `Y`"
///   (substitution notation).
///
/// Some examples of receiver types and their required obligation:
/// - `&'a mut self` requires `&'a mut Self: DispatchFromDyn<&'a mut dyn Trait>`,
/// - `self: Rc<Self>` requires `Rc<Self>: DispatchFromDyn<Rc<dyn Trait>>`,
/// - `self: Pin<Box<Self>>` requires `Pin<Box<Self>>: DispatchFromDyn<Pin<Box<dyn Trait>>>`.
///
/// The only case where the receiver is not dispatchable, but is still a valid receiver
/// type (just not object-safe), is when there is more than one level of pointer indirection.
/// E.g., `self: &&Self`, `self: &Rc<Self>`, `self: Box<Box<Self>>`. In these cases, there
/// is no way, or at least no inexpensive way, to coerce the receiver from the version where
/// `Self = dyn Trait` to the version where `Self = T`, where `T` is the unknown erased type
/// contained by the trait object, because the object that needs to be coerced is behind
/// a pointer.
///
/// In practice, we cannot use `dyn Trait` explicitly in the obligation because it would result
/// in a new check that `Trait` is object safe, creating a cycle (until object_safe_for_dispatch
/// is stabilized, see tracking issue https://github.com/rust-lang/rust/issues/43561).
/// Instead, we fudge a little by introducing a new type parameter `U` such that
/// `Self: Unsize<U>` and `U: Trait + ?Sized`, and use `U` in place of `dyn Trait`.
/// Written as a chalk-style query:
///
///     forall (U: Trait + ?Sized) {
///         if (Self: Unsize<U>) {
///             Receiver: DispatchFromDyn<Receiver[Self => U]>
///         }
///     }
///
/// for `self: &'a mut Self`, this means `&'a mut Self: DispatchFromDyn<&'a mut U>`
/// for `self: Rc<Self>`, this means `Rc<Self>: DispatchFromDyn<Rc<U>>`
/// for `self: Pin<Box<Self>>`, this means `Pin<Box<Self>>: DispatchFromDyn<Pin<Box<U>>>`
//
// FIXME(mikeyhew) when unsized receivers are implemented as part of unsized rvalues, add this
// fallback query: `Receiver: Unsize<Receiver[Self => U]>` to support receivers like
// `self: Wrapper<Self>`.
#[allow(dead_code)]
fn receiver_is_dispatchable<'tcx>(
    tcx: TyCtxt<'tcx>,
    method: &ty::AssocItem,
    receiver_ty: Ty<'tcx>,
) -> bool {
    debug!("receiver_is_dispatchable: method = {:?}, receiver_ty = {:?}", method, receiver_ty);

    let traits = (tcx.lang_items().unsize_trait(), tcx.lang_items().dispatch_from_dyn_trait());
    let (unsize_did, dispatch_from_dyn_did) = if let (Some(u), Some(cu)) = traits {
        (u, cu)
    } else {
        debug!("receiver_is_dispatchable: Missing Unsize or DispatchFromDyn traits");
        return false;
    };

    // the type `U` in the query
    // use a bogus type parameter to mimick a forall(U) query using u32::MAX for now.
    // FIXME(mikeyhew) this is a total hack. Once object_safe_for_dispatch is stabilized, we can
    // replace this with `dyn Trait`
    let unsized_self_ty: Ty<'tcx> =
        tcx.mk_ty_param(::std::u32::MAX, Symbol::intern("RustaceansAreAwesome"));

    // `Receiver[Self => U]`
    let unsized_receiver_ty =
        receiver_for_self_ty(tcx, receiver_ty, unsized_self_ty, method.def_id);

    // create a modified param env, with `Self: Unsize<U>` and `U: Trait` added to caller bounds
    // `U: ?Sized` is already implied here
    let param_env = {
        let mut param_env = tcx.param_env(method.def_id);

        // Self: Unsize<U>
        let unsize_predicate = ty::TraitRef {
            def_id: unsize_did,
            substs: tcx.mk_substs_trait(tcx.types.self_param, &[unsized_self_ty.into()]),
        }
        .to_predicate();

        // U: Trait<Arg1, ..., ArgN>
        let trait_predicate = {
            let substs =
                InternalSubsts::for_item(tcx, method.container.assert_trait(), |param, _| {
                    if param.index == 0 {
                        unsized_self_ty.into()
                    } else {
                        tcx.mk_param_from_def(param)
                    }
                });

            ty::TraitRef { def_id: unsize_did, substs }.to_predicate()
        };

        let caller_bounds: Vec<Predicate<'tcx>> = param_env
            .caller_bounds
            .iter()
            .cloned()
            .chain(iter::once(unsize_predicate))
            .chain(iter::once(trait_predicate))
            .collect();

        param_env.caller_bounds = tcx.intern_predicates(&caller_bounds);

        param_env
    };

    // Receiver: DispatchFromDyn<Receiver[Self => U]>
    let obligation = {
        let predicate = ty::TraitRef {
            def_id: dispatch_from_dyn_did,
            substs: tcx.mk_substs_trait(receiver_ty, &[unsized_receiver_ty.into()]),
        }
        .to_predicate();

        Obligation::new(ObligationCause::dummy(), param_env, predicate)
    };

    tcx.infer_ctxt().enter(|ref infcx| {
        // the receiver is dispatchable iff the obligation holds
        infcx.predicate_must_hold_modulo_regions(&obligation)
    })
}

fn contains_illegal_self_type_reference<'tcx>(
    tcx: TyCtxt<'tcx>,
    trait_def_id: DefId,
    ty: Ty<'tcx>,
) -> bool {
    // This is somewhat subtle. In general, we want to forbid
    // references to `Self` in the argument and return types,
    // since the value of `Self` is erased. However, there is one
    // exception: it is ok to reference `Self` in order to access
    // an associated type of the current trait, since we retain
    // the value of those associated types in the object type
    // itself.
    //
    // ```rust
    // trait SuperTrait {
    //     type X;
    // }
    //
    // trait Trait : SuperTrait {
    //     type Y;
    //     fn foo(&self, x: Self) // bad
    //     fn foo(&self) -> Self // bad
    //     fn foo(&self) -> Option<Self> // bad
    //     fn foo(&self) -> Self::Y // OK, desugars to next example
    //     fn foo(&self) -> <Self as Trait>::Y // OK
    //     fn foo(&self) -> Self::X // OK, desugars to next example
    //     fn foo(&self) -> <Self as SuperTrait>::X // OK
    // }
    // ```
    //
    // However, it is not as simple as allowing `Self` in a projected
    // type, because there are illegal ways to use `Self` as well:
    //
    // ```rust
    // trait Trait : SuperTrait {
    //     ...
    //     fn foo(&self) -> <Self as SomeOtherTrait>::X;
    // }
    // ```
    //
    // Here we will not have the type of `X` recorded in the
    // object type, and we cannot resolve `Self as SomeOtherTrait`
    // without knowing what `Self` is.

    let mut supertraits: Option<Vec<ty::PolyTraitRef<'tcx>>> = None;
    let mut error = false;
    let self_ty = tcx.types.self_param;
    ty.maybe_walk(|ty| {
        match ty.kind {
            ty::Param(_) => {
                if ty == self_ty {
                    error = true;
                }

                false // no contained types to walk
            }

            ty::Projection(ref data) => {
                // This is a projected type `<Foo as SomeTrait>::X`.

                // Compute supertraits of current trait lazily.
                if supertraits.is_none() {
                    let trait_ref = ty::Binder::bind(ty::TraitRef::identity(tcx, trait_def_id));
                    supertraits = Some(traits::supertraits(tcx, trait_ref).collect());
                }

                // Determine whether the trait reference `Foo as
                // SomeTrait` is in fact a supertrait of the
                // current trait. In that case, this type is
                // legal, because the type `X` will be specified
                // in the object type.  Note that we can just use
                // direct equality here because all of these types
                // are part of the formal parameter listing, and
                // hence there should be no inference variables.
                let projection_trait_ref = ty::Binder::bind(data.trait_ref(tcx));
                let is_supertrait_of_current_trait =
                    supertraits.as_ref().unwrap().contains(&projection_trait_ref);

                if is_supertrait_of_current_trait {
                    false // do not walk contained types, do not report error, do collect $200
                } else {
                    true // DO walk contained types, POSSIBLY reporting an error
                }
            }

            _ => true, // walk contained types, if any
        }
    });

    error
}

pub(super) fn is_object_safe_provider(tcx: TyCtxt<'_>, trait_def_id: DefId) -> bool {
    tcx.object_safety_violations(trait_def_id).is_empty()
}
