// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! "Object safety" refers to the ability for a trait to be converted
//! to an object. In general, traits may only be converted to an
//! object if all of their methods meet certain criteria. In particular,
//! they must:
//!
//!   - have a suitable receiver from which we can extract a vtable and coerce to a "thin" version
//!     that doesn't contain the vtable;
//!   - not reference the erased type `Self` except for in this receiver;
//!   - not have generic type parameters

use super::elaborate_predicates;

use hir::def_id::DefId;
use lint;
use traits::{self, Obligation, ObligationCause};
use ty::{self, Ty, TyCtxt, TypeFoldable, Predicate, ToPredicate};
use ty::subst::{Subst, Substs};
use std::borrow::Cow;
use std::iter::{self};
use syntax::ast::{self, Name};
use syntax_pos::Span;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectSafetyViolation {
    /// Self : Sized declared on the trait
    SizedSelf,

    /// Supertrait reference references `Self` an in illegal location
    /// (e.g. `trait Foo : Bar<Self>`)
    SupertraitSelf,

    /// Method has something illegal
    Method(ast::Name, MethodViolationCode),

    /// Associated const
    AssociatedConst(ast::Name),
}

impl ObjectSafetyViolation {
    pub fn error_msg(&self) -> Cow<'static, str> {
        match *self {
            ObjectSafetyViolation::SizedSelf =>
                "the trait cannot require that `Self : Sized`".into(),
            ObjectSafetyViolation::SupertraitSelf =>
                "the trait cannot use `Self` as a type parameter \
                 in the supertraits or where-clauses".into(),
            ObjectSafetyViolation::Method(name, MethodViolationCode::StaticMethod) =>
                format!("method `{}` has no receiver", name).into(),
            ObjectSafetyViolation::Method(name, MethodViolationCode::ReferencesSelf) =>
                format!("method `{}` references the `Self` type \
                         in its arguments or return type", name).into(),
            ObjectSafetyViolation::Method(name,
                                            MethodViolationCode::WhereClauseReferencesSelf(_)) =>
                format!("method `{}` references the `Self` type in where clauses", name).into(),
            ObjectSafetyViolation::Method(name, MethodViolationCode::Generic) =>
                format!("method `{}` has generic type parameters", name).into(),
            ObjectSafetyViolation::Method(name, MethodViolationCode::UncoercibleReceiver) =>
                format!("method `{}` has an uncoercible receiver type", name).into(),
            ObjectSafetyViolation::AssociatedConst(name) =>
                format!("the trait cannot contain associated consts like `{}`", name).into(),
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

    /// e.g. `fn foo(&self) where Self: Clone`
    WhereClauseReferencesSelf(Span),

    /// e.g., `fn foo<A>()`
    Generic,

    /// the self argument can't be coerced from Self=dyn Trait to Self=T where T: Trait
    UncoercibleReceiver,
}

impl<'a, 'tcx> TyCtxt<'a, 'tcx, 'tcx> {

    /// Returns the object safety violations that affect
    /// astconv - currently, Self in supertraits. This is needed
    /// because `object_safety_violations` can't be used during
    /// type collection.
    pub fn astconv_object_safety_violations(self, trait_def_id: DefId)
                                            -> Vec<ObjectSafetyViolation>
    {
        let violations = traits::supertrait_def_ids(self, trait_def_id)
            .filter(|&def_id| self.predicates_reference_self(def_id, true))
            .map(|_| ObjectSafetyViolation::SupertraitSelf)
            .collect();

        debug!("astconv_object_safety_violations(trait_def_id={:?}) = {:?}",
               trait_def_id,
               violations);

        violations
    }

    pub fn object_safety_violations(self, trait_def_id: DefId)
                                    -> Vec<ObjectSafetyViolation>
    {
        debug!("object_safety_violations: {:?}", trait_def_id);

        traits::supertrait_def_ids(self, trait_def_id)
            .flat_map(|def_id| self.object_safety_violations_for_trait(def_id))
            .collect()
    }

    fn object_safety_violations_for_trait(self, trait_def_id: DefId)
                                          -> Vec<ObjectSafetyViolation>
    {
        // Check methods for violations.
        let mut violations: Vec<_> = self.associated_items(trait_def_id)
            .filter(|item| item.kind == ty::AssociatedKind::Method)
            .filter_map(|item|
                self.object_safety_violation_for_method(trait_def_id, &item)
                    .map(|code| ObjectSafetyViolation::Method(item.ident.name, code))
            ).filter(|violation| {
                if let ObjectSafetyViolation::Method(_,
                    MethodViolationCode::WhereClauseReferencesSelf(span)) = violation
                {
                    // Using `CRATE_NODE_ID` is wrong, but it's hard to get a more precise id.
                    // It's also hard to get a use site span, so we use the method definition span.
                    self.lint_node_note(
                        lint::builtin::WHERE_CLAUSES_OBJECT_SAFETY,
                        ast::CRATE_NODE_ID,
                        *span,
                        &format!("the trait `{}` cannot be made into an object",
                                 self.item_path_str(trait_def_id)),
                        &violation.error_msg());
                    false
                } else {
                    true
                }
            }).collect();

        // Check the trait itself.
        if self.trait_has_sized_self(trait_def_id) {
            violations.push(ObjectSafetyViolation::SizedSelf);
        }
        if self.predicates_reference_self(trait_def_id, false) {
            violations.push(ObjectSafetyViolation::SupertraitSelf);
        }

        violations.extend(self.associated_items(trait_def_id)
            .filter(|item| item.kind == ty::AssociatedKind::Const)
            .map(|item| ObjectSafetyViolation::AssociatedConst(item.ident.name)));

        debug!("object_safety_violations_for_trait(trait_def_id={:?}) = {:?}",
               trait_def_id,
               violations);

        violations
    }

    fn predicates_reference_self(
        self,
        trait_def_id: DefId,
        supertraits_only: bool) -> bool
    {
        let trait_ref = ty::Binder::dummy(ty::TraitRef::identity(self, trait_def_id));
        let predicates = if supertraits_only {
            self.super_predicates_of(trait_def_id)
        } else {
            self.predicates_of(trait_def_id)
        };
        predicates
            .predicates
            .into_iter()
            .map(|(predicate, _)| predicate.subst_supertrait(self, &trait_ref))
            .any(|predicate| {
                match predicate {
                    ty::Predicate::Trait(ref data) => {
                        // In the case of a trait predicate, we can skip the "self" type.
                        data.skip_binder().input_types().skip(1).any(|t| t.has_self_ty())
                    }
                    ty::Predicate::Projection(..) |
                    ty::Predicate::WellFormed(..) |
                    ty::Predicate::ObjectSafe(..) |
                    ty::Predicate::TypeOutlives(..) |
                    ty::Predicate::RegionOutlives(..) |
                    ty::Predicate::ClosureKind(..) |
                    ty::Predicate::Subtype(..) |
                    ty::Predicate::ConstEvaluatable(..) => {
                        false
                    }
                }
            })
    }

    fn trait_has_sized_self(self, trait_def_id: DefId) -> bool {
        self.generics_require_sized_self(trait_def_id)
    }

    fn generics_require_sized_self(self, def_id: DefId) -> bool {
        let sized_def_id = match self.lang_items().sized_trait() {
            Some(def_id) => def_id,
            None => { return false; /* No Sized trait, can't require it! */ }
        };

        // Search for a predicate like `Self : Sized` amongst the trait bounds.
        let predicates = self.predicates_of(def_id);
        let predicates = predicates.instantiate_identity(self).predicates;
        elaborate_predicates(self, predicates)
            .any(|predicate| match predicate {
                ty::Predicate::Trait(ref trait_pred) if trait_pred.def_id() == sized_def_id => {
                    trait_pred.skip_binder().self_ty().is_self()
                }
                ty::Predicate::Projection(..) |
                ty::Predicate::Trait(..) |
                ty::Predicate::Subtype(..) |
                ty::Predicate::RegionOutlives(..) |
                ty::Predicate::WellFormed(..) |
                ty::Predicate::ObjectSafe(..) |
                ty::Predicate::ClosureKind(..) |
                ty::Predicate::TypeOutlives(..) |
                ty::Predicate::ConstEvaluatable(..) => {
                    false
                }
            }
        )
    }

    /// Returns `Some(_)` if this method makes the containing trait not object safe.
    fn object_safety_violation_for_method(self,
                                          trait_def_id: DefId,
                                          method: &ty::AssociatedItem)
                                          -> Option<MethodViolationCode>
    {
        // Any method that has a `Self : Sized` requisite is otherwise
        // exempt from the regulations.
        if self.generics_require_sized_self(method.def_id) {
            return None;
        }

        self.virtual_call_violation_for_method(trait_def_id, method)
    }

    /// We say a method is *vtable safe* if it can be invoked on a trait
    /// object.  Note that object-safe traits can have some
    /// non-vtable-safe methods, so long as they require `Self:Sized` or
    /// otherwise ensure that they cannot be used when `Self=Trait`.
    pub fn is_vtable_safe_method(self,
                                 trait_def_id: DefId,
                                 method: &ty::AssociatedItem)
                                 -> bool
    {
        // Any method that has a `Self : Sized` requisite can't be called.
        if self.generics_require_sized_self(method.def_id) {
            return false;
        }

        match self.virtual_call_violation_for_method(trait_def_id, method) {
            None | Some(MethodViolationCode::WhereClauseReferencesSelf(_)) => true,
            Some(_) => false,
        }
    }

    /// Returns `Some(_)` if this method cannot be called on a trait
    /// object; this does not necessarily imply that the enclosing trait
    /// is not object safe, because the method might have a where clause
    /// `Self:Sized`.
    fn virtual_call_violation_for_method(self,
                                         trait_def_id: DefId,
                                         method: &ty::AssociatedItem)
                                         -> Option<MethodViolationCode>
    {
        // The method's first parameter must be named `self`
        if !method.method_has_self_argument {
            return Some(MethodViolationCode::StaticMethod);
        }

        let sig = self.fn_sig(method.def_id);

        for input_ty in &sig.skip_binder().inputs()[1..] {
            if self.contains_illegal_self_type_reference(trait_def_id, input_ty) {
                return Some(MethodViolationCode::ReferencesSelf);
            }
        }
        if self.contains_illegal_self_type_reference(trait_def_id, sig.output().skip_binder()) {
            return Some(MethodViolationCode::ReferencesSelf);
        }

        // We can't monomorphize things like `fn foo<A>(...)`.
        if self.generics_of(method.def_id).own_counts().types != 0 {
            return Some(MethodViolationCode::Generic);
        }

        if self.predicates_of(method.def_id).predicates.into_iter()
                // A trait object can't claim to live more than the concrete type,
                // so outlives predicates will always hold.
                .filter(|(p, _)| p.to_opt_type_outlives().is_none())
                .collect::<Vec<_>>()
                // Do a shallow visit so that `contains_illegal_self_type_reference`
                // may apply it's custom visiting.
                .visit_tys_shallow(|t| self.contains_illegal_self_type_reference(trait_def_id, t)) {
            let span = self.def_span(method.def_id);
            return Some(MethodViolationCode::WhereClauseReferencesSelf(span));
        }

        let receiver_ty = self.liberate_late_bound_regions(
            method.def_id,
            &sig.map_bound(|sig| sig.inputs()[0]),
        );

        // until `unsized_locals` is fully implemented, `self: Self` can't be coerced from
        // `Self=dyn Trait` to `Self=T`. However, this is already considered object-safe. We allow
        // it as a special case here.
        // FIXME(mikeyhew) get rid of this `if` statement once `receiver_is_coercible` allows
        // `Receiver: Unsize<Receiver[Self => dyn Trait]>`
        if receiver_ty != self.mk_self_type() {
            if !self.receiver_is_coercible(method, receiver_ty) {
                return Some(MethodViolationCode::UncoercibleReceiver);
            }
        }

        None
    }

    /// checks the method's receiver (the `self` argument) can be coerced from
    /// a fat pointer, including the trait object vtable, to a thin pointer.
    /// e.g. from `Rc<dyn Trait>` to `Rc<T>`, where `T` is the erased type of the underlying object.
    /// More formally:
    /// - let `Receiver` be the type of the `self` argument, i.e `Self`, `&Self`, `Rc<Self>`
    /// - require the following bound:
    ///       forall(T: Trait) {
    ///           Receiver[Self => dyn Trait]: CoerceSized<Receiver[Self => T]>
    ///       }
    ///   where `Foo[X => Y]` means "the same type as `Foo`, but with `X` replaced with `Y`"
    ///   (substitution notation).
    ///
    /// some examples of receiver types and their required obligation
    /// - `&'a mut self` requires `&'a mut dyn Trait: CoerceSized<&'a mut T>`
    /// - `self: Rc<Self>` requires `Rc<dyn Trait>: CoerceSized<Rc<T>>`
    ///
    /// The only case where the receiver is not coercible, but is still a valid receiver
    /// type (just not object-safe), is when there is more than one level of pointer indirection.
    /// e.g. `self: &&Self`, `self: &Rc<Self>`, `self: Box<Box<Self>>`. In these cases, there
    /// is no way, or at least no inexpensive way, to coerce the receiver, because the object that
    /// needs to be coerced is behind a pointer.
    ///
    /// In practice, there are issues with the above bound: `where` clauses that apply to `Self`
    /// would have to apply to `T`, trait object types have a lot of parameters that need to
    /// be filled in (lifetime and type parameters, and the lifetime of the actual object), and
    /// I'm pretty sure using `dyn Trait` in the query causes another object-safety query for
    /// `Trait`, resulting in cyclic queries. So in the implementation, we use the following,
    /// more general bound:
    ///
    ///     forall (U: ?Sized) {
    ///         if (Self: Unsize<U>) {
    ///             Receiver[Self => U]: CoerceSized<Receiver>
    ///         }
    ///     }
    ///
    /// for `self: &'a mut Self`, this means `&'a mut U: CoerceSized<&'a mut Self>`
    /// for `self: Rc<Self>`, this means `Rc<U>: CoerceSized<Rc<Self>>`
    //
    // FIXME(mikeyhew) when unsized receivers are implemented as part of unsized rvalues, add this
    // fallback query: `Receiver: Unsize<Receiver[Self => U]>` to support receivers like
    // `self: Wrapper<Self>`.
    #[allow(dead_code)]
    fn receiver_is_coercible(
        self,
        method: &ty::AssociatedItem,
        receiver_ty: Ty<'tcx>,
    ) -> bool {
        debug!("receiver_is_coercible: method = {:?}, receiver_ty = {:?}", method, receiver_ty);

        let traits = (self.lang_items().unsize_trait(),
                      self.lang_items().coerce_sized_trait());
        let (unsize_did, coerce_sized_did) = if let (Some(u), Some(cu)) = traits {
            (u, cu)
        } else {
            debug!("receiver_is_coercible: Missing Unsize or CoerceSized traits");
            return false;
        };

        // use a bogus type parameter to mimick a forall(U) query using u32::MAX for now.
        // FIXME(mikeyhew) this is a total hack, and we should replace it when real forall queries
        // are implemented
        let target_self_ty: Ty<'tcx> = self.mk_ty_param(
            ::std::u32::MAX,
            Name::intern("RustaceansAreAwesome").as_interned_str(),
        );

        // create a modified param env, with `Self: Unsize<U>` added to the caller bounds
        let param_env = {
            let mut param_env = self.param_env(method.def_id);

            let predicate = ty::TraitRef {
                def_id: unsize_did,
                substs: self.mk_substs_trait(self.mk_self_type(), &[target_self_ty.into()]),
            }.to_predicate();

            let caller_bounds: Vec<Predicate<'tcx>> = param_env.caller_bounds.iter().cloned()
                .chain(iter::once(predicate))
                .collect();

            param_env.caller_bounds = self.intern_predicates(&caller_bounds);

            param_env
        };

        let receiver_substs = Substs::for_item(self, method.def_id, |param, _| {
            if param.index == 0 {
                target_self_ty.into()
            } else {
                self.mk_param_from_def(param)
            }
        });
        // the type `Receiver[Self => U]` in the query
        let unsized_receiver_ty = receiver_ty.subst(self, receiver_substs);

        // Receiver[Self => U]: CoerceSized<Receiver>
        let obligation = {
            let predicate = ty::TraitRef {
                def_id: coerce_sized_did,
                substs: self.mk_substs_trait(unsized_receiver_ty, &[receiver_ty.into()]),
            }.to_predicate();

            Obligation::new(
                ObligationCause::dummy(),
                param_env,
                predicate,
            )
        };

        self.infer_ctxt().enter(|ref infcx| {
            // the receiver is coercible iff the obligation holds
            infcx.predicate_must_hold(&obligation)
        })
    }

    fn contains_illegal_self_type_reference(self,
                                            trait_def_id: DefId,
                                            ty: Ty<'tcx>)
                                            -> bool
    {
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
        ty.maybe_walk(|ty| {
            match ty.sty {
                ty::Param(ref param_ty) => {
                    if param_ty.is_self() {
                        error = true;
                    }

                    false // no contained types to walk
                }

                ty::Projection(ref data) => {
                    // This is a projected type `<Foo as SomeTrait>::X`.

                    // Compute supertraits of current trait lazily.
                    if supertraits.is_none() {
                        let trait_ref = ty::Binder::bind(
                            ty::TraitRef::identity(self, trait_def_id),
                        );
                        supertraits = Some(traits::supertraits(self, trait_ref).collect());
                    }

                    // Determine whether the trait reference `Foo as
                    // SomeTrait` is in fact a supertrait of the
                    // current trait. In that case, this type is
                    // legal, because the type `X` will be specified
                    // in the object type.  Note that we can just use
                    // direct equality here because all of these types
                    // are part of the formal parameter listing, and
                    // hence there should be no inference variables.
                    let projection_trait_ref = ty::Binder::bind(data.trait_ref(self));
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
}

pub(super) fn is_object_safe_provider<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                                trait_def_id: DefId) -> bool {
    tcx.object_safety_violations(trait_def_id).is_empty()
}
