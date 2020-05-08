//! Generalized type relating mechanism.
//!
//! A type relation `R` relates a pair of values `(A, B)`. `A and B` are usually
//! types or regions but can be other things. Examples of type relations are
//! subtyping, type equality, etc.

use crate::mir::interpret::{get_slice_bytes, ConstValue};
use crate::ty::error::{ExpectedFound, TypeError};
use crate::ty::subst::{GenericArg, GenericArgKind, SubstsRef};
use crate::ty::{self, Ty, TyCtxt, TypeFoldable};
use rustc_hir as ast;
use rustc_hir::def_id::DefId;
use rustc_span::DUMMY_SP;
use rustc_target::spec::abi;
use std::iter;
use std::rc::Rc;

pub type RelateResult<'tcx, T> = Result<T, TypeError<'tcx>>;

#[derive(Clone, Debug)]
pub enum Cause {
    ExistentialRegionBound, // relating an existential region bound
}

pub trait TypeRelation<'tcx>: Sized {
    fn tcx(&self) -> TyCtxt<'tcx>;

    fn param_env(&self) -> ty::ParamEnv<'tcx>;

    /// Returns a static string we can use for printouts.
    fn tag(&self) -> &'static str;

    /// Returns `true` if the value `a` is the "expected" type in the
    /// relation. Just affects error messages.
    fn a_is_expected(&self) -> bool;

    fn with_cause<F, R>(&mut self, _cause: Cause, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        f(self)
    }

    /// Generic relation routine suitable for most anything.
    fn relate<T: Relate<'tcx>>(&mut self, a: &T, b: &T) -> RelateResult<'tcx, T> {
        Relate::relate(self, a, b)
    }

    /// Relate the two substitutions for the given item. The default
    /// is to look up the variance for the item and proceed
    /// accordingly.
    fn relate_item_substs(
        &mut self,
        item_def_id: DefId,
        a_subst: SubstsRef<'tcx>,
        b_subst: SubstsRef<'tcx>,
    ) -> RelateResult<'tcx, SubstsRef<'tcx>> {
        debug!(
            "relate_item_substs(item_def_id={:?}, a_subst={:?}, b_subst={:?})",
            item_def_id, a_subst, b_subst
        );

        let opt_variances = self.tcx().variances_of(item_def_id);
        relate_substs(self, Some(opt_variances), a_subst, b_subst)
    }

    /// Switch variance for the purpose of relating `a` and `b`.
    fn relate_with_variance<T: Relate<'tcx>>(
        &mut self,
        variance: ty::Variance,
        a: &T,
        b: &T,
    ) -> RelateResult<'tcx, T>;

    // Overridable relations. You shouldn't typically call these
    // directly, instead call `relate()`, which in turn calls
    // these. This is both more uniform but also allows us to add
    // additional hooks for other types in the future if needed
    // without making older code, which called `relate`, obsolete.

    fn tys(&mut self, a: Ty<'tcx>, b: Ty<'tcx>) -> RelateResult<'tcx, Ty<'tcx>>;

    fn regions(
        &mut self,
        a: ty::Region<'tcx>,
        b: ty::Region<'tcx>,
    ) -> RelateResult<'tcx, ty::Region<'tcx>>;

    fn consts(
        &mut self,
        a: &'tcx ty::Const<'tcx>,
        b: &'tcx ty::Const<'tcx>,
    ) -> RelateResult<'tcx, &'tcx ty::Const<'tcx>>;

    fn binders<T>(
        &mut self,
        a: &ty::Binder<T>,
        b: &ty::Binder<T>,
    ) -> RelateResult<'tcx, ty::Binder<T>>
    where
        T: Relate<'tcx>;
}

pub trait Relate<'tcx>: TypeFoldable<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Self,
        b: &Self,
    ) -> RelateResult<'tcx, Self>;
}

///////////////////////////////////////////////////////////////////////////
// Relate impls

impl<'tcx> Relate<'tcx> for ty::TypeAndMut<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::TypeAndMut<'tcx>,
        b: &ty::TypeAndMut<'tcx>,
    ) -> RelateResult<'tcx, ty::TypeAndMut<'tcx>> {
        debug!("{}.mts({:?}, {:?})", relation.tag(), a, b);
        if a.mutbl != b.mutbl {
            Err(TypeError::Mutability)
        } else {
            let mutbl = a.mutbl;
            let variance = match mutbl {
                ast::Mutability::Not => ty::Covariant,
                ast::Mutability::Mut => ty::Invariant,
            };
            let ty = relation.relate_with_variance(variance, &a.ty, &b.ty)?;
            Ok(ty::TypeAndMut { ty, mutbl })
        }
    }
}

pub fn relate_substs<R: TypeRelation<'tcx>>(
    relation: &mut R,
    variances: Option<&[ty::Variance]>,
    a_subst: SubstsRef<'tcx>,
    b_subst: SubstsRef<'tcx>,
) -> RelateResult<'tcx, SubstsRef<'tcx>> {
    let tcx = relation.tcx();

    let params = a_subst.iter().zip(b_subst).enumerate().map(|(i, (a, b))| {
        let variance = variances.map_or(ty::Invariant, |v| v[i]);
        relation.relate_with_variance(variance, a, b)
    });

    Ok(tcx.mk_substs(params)?)
}

impl<'tcx> Relate<'tcx> for ty::FnSig<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::FnSig<'tcx>,
        b: &ty::FnSig<'tcx>,
    ) -> RelateResult<'tcx, ty::FnSig<'tcx>> {
        let tcx = relation.tcx();

        if a.c_variadic != b.c_variadic {
            return Err(TypeError::VariadicMismatch(expected_found(
                relation,
                &a.c_variadic,
                &b.c_variadic,
            )));
        }
        let unsafety = relation.relate(&a.unsafety, &b.unsafety)?;
        let abi = relation.relate(&a.abi, &b.abi)?;

        if a.inputs().len() != b.inputs().len() {
            return Err(TypeError::ArgCount);
        }

        let inputs_and_output = a
            .inputs()
            .iter()
            .cloned()
            .zip(b.inputs().iter().cloned())
            .map(|x| (x, false))
            .chain(iter::once(((a.output(), b.output()), true)))
            .map(|((a, b), is_output)| {
                if is_output {
                    relation.relate(&a, &b)
                } else {
                    relation.relate_with_variance(ty::Contravariant, &a, &b)
                }
            });
        Ok(ty::FnSig {
            inputs_and_output: tcx.mk_type_list(inputs_and_output)?,
            c_variadic: a.c_variadic,
            unsafety,
            abi,
        })
    }
}

impl<'tcx> Relate<'tcx> for ast::Unsafety {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ast::Unsafety,
        b: &ast::Unsafety,
    ) -> RelateResult<'tcx, ast::Unsafety> {
        if a != b {
            Err(TypeError::UnsafetyMismatch(expected_found(relation, a, b)))
        } else {
            Ok(*a)
        }
    }
}

impl<'tcx> Relate<'tcx> for abi::Abi {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &abi::Abi,
        b: &abi::Abi,
    ) -> RelateResult<'tcx, abi::Abi> {
        if a == b { Ok(*a) } else { Err(TypeError::AbiMismatch(expected_found(relation, a, b))) }
    }
}

impl<'tcx> Relate<'tcx> for ty::ProjectionTy<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::ProjectionTy<'tcx>,
        b: &ty::ProjectionTy<'tcx>,
    ) -> RelateResult<'tcx, ty::ProjectionTy<'tcx>> {
        if a.item_def_id != b.item_def_id {
            Err(TypeError::ProjectionMismatched(expected_found(
                relation,
                &a.item_def_id,
                &b.item_def_id,
            )))
        } else {
            let substs = relation.relate(&a.substs, &b.substs)?;
            Ok(ty::ProjectionTy { item_def_id: a.item_def_id, substs: &substs })
        }
    }
}

impl<'tcx> Relate<'tcx> for ty::ExistentialProjection<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::ExistentialProjection<'tcx>,
        b: &ty::ExistentialProjection<'tcx>,
    ) -> RelateResult<'tcx, ty::ExistentialProjection<'tcx>> {
        if a.item_def_id != b.item_def_id {
            Err(TypeError::ProjectionMismatched(expected_found(
                relation,
                &a.item_def_id,
                &b.item_def_id,
            )))
        } else {
            let ty = relation.relate(&a.ty, &b.ty)?;
            let substs = relation.relate(&a.substs, &b.substs)?;
            Ok(ty::ExistentialProjection { item_def_id: a.item_def_id, substs, ty })
        }
    }
}

impl<'tcx> Relate<'tcx> for Vec<ty::PolyExistentialProjection<'tcx>> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Vec<ty::PolyExistentialProjection<'tcx>>,
        b: &Vec<ty::PolyExistentialProjection<'tcx>>,
    ) -> RelateResult<'tcx, Vec<ty::PolyExistentialProjection<'tcx>>> {
        // To be compatible, `a` and `b` must be for precisely the
        // same set of traits and item names. We always require that
        // projection bounds lists are sorted by trait-def-id and item-name,
        // so we can just iterate through the lists pairwise, so long as they are the
        // same length.
        if a.len() != b.len() {
            Err(TypeError::ProjectionBoundsLength(expected_found(relation, &a.len(), &b.len())))
        } else {
            a.iter().zip(b).map(|(a, b)| relation.relate(a, b)).collect()
        }
    }
}

impl<'tcx> Relate<'tcx> for ty::TraitRef<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::TraitRef<'tcx>,
        b: &ty::TraitRef<'tcx>,
    ) -> RelateResult<'tcx, ty::TraitRef<'tcx>> {
        // Different traits cannot be related.
        if a.def_id != b.def_id {
            Err(TypeError::Traits(expected_found(relation, &a.def_id, &b.def_id)))
        } else {
            let substs = relate_substs(relation, None, a.substs, b.substs)?;
            Ok(ty::TraitRef { def_id: a.def_id, substs })
        }
    }
}

impl<'tcx> Relate<'tcx> for ty::ExistentialTraitRef<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::ExistentialTraitRef<'tcx>,
        b: &ty::ExistentialTraitRef<'tcx>,
    ) -> RelateResult<'tcx, ty::ExistentialTraitRef<'tcx>> {
        // Different traits cannot be related.
        if a.def_id != b.def_id {
            Err(TypeError::Traits(expected_found(relation, &a.def_id, &b.def_id)))
        } else {
            let substs = relate_substs(relation, None, a.substs, b.substs)?;
            Ok(ty::ExistentialTraitRef { def_id: a.def_id, substs })
        }
    }
}

#[derive(Debug, Clone, TypeFoldable)]
struct GeneratorWitness<'tcx>(&'tcx ty::List<Ty<'tcx>>);

impl<'tcx> Relate<'tcx> for GeneratorWitness<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &GeneratorWitness<'tcx>,
        b: &GeneratorWitness<'tcx>,
    ) -> RelateResult<'tcx, GeneratorWitness<'tcx>> {
        assert_eq!(a.0.len(), b.0.len());
        let tcx = relation.tcx();
        let types = tcx.mk_type_list(a.0.iter().zip(b.0).map(|(a, b)| relation.relate(a, b)))?;
        Ok(GeneratorWitness(types))
    }
}

impl<'tcx> Relate<'tcx> for Ty<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Ty<'tcx>,
        b: &Ty<'tcx>,
    ) -> RelateResult<'tcx, Ty<'tcx>> {
        relation.tys(a, b)
    }
}

/// The main "type relation" routine. Note that this does not handle
/// inference artifacts, so you should filter those out before calling
/// it.
pub fn super_relate_tys<R: TypeRelation<'tcx>>(
    relation: &mut R,
    a: Ty<'tcx>,
    b: Ty<'tcx>,
) -> RelateResult<'tcx, Ty<'tcx>> {
    let tcx = relation.tcx();
    debug!("super_relate_tys: a={:?} b={:?}", a, b);
    match (&a.kind, &b.kind) {
        (&ty::Infer(_), _) | (_, &ty::Infer(_)) => {
            // The caller should handle these cases!
            bug!("var types encountered in super_relate_tys")
        }

        (ty::Bound(..), _) | (_, ty::Bound(..)) => {
            bug!("bound types encountered in super_relate_tys")
        }

        (&ty::Error, _) | (_, &ty::Error) => Ok(tcx.types.err),

        (&ty::Never, _)
        | (&ty::Char, _)
        | (&ty::Bool, _)
        | (&ty::Int(_), _)
        | (&ty::Uint(_), _)
        | (&ty::Float(_), _)
        | (&ty::Str, _)
            if a == b =>
        {
            Ok(a)
        }

        (&ty::Param(ref a_p), &ty::Param(ref b_p)) if a_p.index == b_p.index => Ok(a),

        (ty::Placeholder(p1), ty::Placeholder(p2)) if p1 == p2 => Ok(a),

        (&ty::Adt(a_def, a_substs), &ty::Adt(b_def, b_substs)) if a_def == b_def => {
            let substs = relation.relate_item_substs(a_def.did, a_substs, b_substs)?;
            Ok(tcx.mk_adt(a_def, substs))
        }

        (&ty::Foreign(a_id), &ty::Foreign(b_id)) if a_id == b_id => Ok(tcx.mk_foreign(a_id)),

        (&ty::Dynamic(ref a_obj, ref a_region), &ty::Dynamic(ref b_obj, ref b_region)) => {
            let region_bound = relation.with_cause(Cause::ExistentialRegionBound, |relation| {
                relation.relate_with_variance(ty::Contravariant, a_region, b_region)
            })?;
            Ok(tcx.mk_dynamic(relation.relate(a_obj, b_obj)?, region_bound))
        }

        (&ty::Generator(a_id, a_substs, movability), &ty::Generator(b_id, b_substs, _))
            if a_id == b_id =>
        {
            // All Generator types with the same id represent
            // the (anonymous) type of the same generator expression. So
            // all of their regions should be equated.
            let substs = relation.relate(&a_substs, &b_substs)?;
            Ok(tcx.mk_generator(a_id, substs, movability))
        }

        (&ty::GeneratorWitness(a_types), &ty::GeneratorWitness(b_types)) => {
            // Wrap our types with a temporary GeneratorWitness struct
            // inside the binder so we can related them
            let a_types = a_types.map_bound(GeneratorWitness);
            let b_types = b_types.map_bound(GeneratorWitness);
            // Then remove the GeneratorWitness for the result
            let types = relation.relate(&a_types, &b_types)?.map_bound(|witness| witness.0);
            Ok(tcx.mk_generator_witness(types))
        }

        (&ty::Closure(a_id, a_substs), &ty::Closure(b_id, b_substs)) if a_id == b_id => {
            // All Closure types with the same id represent
            // the (anonymous) type of the same closure expression. So
            // all of their regions should be equated.
            let substs = relation.relate(&a_substs, &b_substs)?;
            Ok(tcx.mk_closure(a_id, &substs))
        }

        (&ty::RawPtr(ref a_mt), &ty::RawPtr(ref b_mt)) => {
            let mt = relation.relate(a_mt, b_mt)?;
            Ok(tcx.mk_ptr(mt))
        }

        (&ty::Ref(a_r, a_ty, a_mutbl), &ty::Ref(b_r, b_ty, b_mutbl)) => {
            let r = relation.relate_with_variance(ty::Contravariant, &a_r, &b_r)?;
            let a_mt = ty::TypeAndMut { ty: a_ty, mutbl: a_mutbl };
            let b_mt = ty::TypeAndMut { ty: b_ty, mutbl: b_mutbl };
            let mt = relation.relate(&a_mt, &b_mt)?;
            Ok(tcx.mk_ref(r, mt))
        }

        (&ty::Array(a_t, sz_a), &ty::Array(b_t, sz_b)) => {
            let t = relation.relate(&a_t, &b_t)?;
            match relation.relate(&sz_a, &sz_b) {
                Ok(sz) => Ok(tcx.mk_ty(ty::Array(t, sz))),
                Err(err) => {
                    // Check whether the lengths are both concrete/known values,
                    // but are unequal, for better diagnostics.
                    let sz_a = sz_a.try_eval_usize(tcx, relation.param_env());
                    let sz_b = sz_b.try_eval_usize(tcx, relation.param_env());
                    match (sz_a, sz_b) {
                        (Some(sz_a_val), Some(sz_b_val)) => Err(TypeError::FixedArraySize(
                            expected_found(relation, &sz_a_val, &sz_b_val),
                        )),
                        _ => Err(err),
                    }
                }
            }
        }

        (&ty::Slice(a_t), &ty::Slice(b_t)) => {
            let t = relation.relate(&a_t, &b_t)?;
            Ok(tcx.mk_slice(t))
        }

        (&ty::Tuple(as_), &ty::Tuple(bs)) => {
            if as_.len() == bs.len() {
                Ok(tcx.mk_tup(
                    as_.iter()
                        .zip(bs)
                        .map(|(a, b)| relation.relate(&a.expect_ty(), &b.expect_ty())),
                )?)
            } else if !(as_.is_empty() || bs.is_empty()) {
                Err(TypeError::TupleSize(expected_found(relation, &as_.len(), &bs.len())))
            } else {
                Err(TypeError::Sorts(expected_found(relation, &a, &b)))
            }
        }

        (&ty::FnDef(a_def_id, a_substs), &ty::FnDef(b_def_id, b_substs))
            if a_def_id == b_def_id =>
        {
            let substs = relation.relate_item_substs(a_def_id, a_substs, b_substs)?;
            Ok(tcx.mk_fn_def(a_def_id, substs))
        }

        (&ty::FnPtr(a_fty), &ty::FnPtr(b_fty)) => {
            let fty = relation.relate(&a_fty, &b_fty)?;
            Ok(tcx.mk_fn_ptr(fty))
        }

        (ty::UnnormalizedProjection(a_data), ty::UnnormalizedProjection(b_data)) => {
            let projection_ty = relation.relate(a_data, b_data)?;
            Ok(tcx.mk_ty(ty::UnnormalizedProjection(projection_ty)))
        }

        // these two are already handled downstream in case of lazy normalization
        (ty::Projection(a_data), ty::Projection(b_data)) => {
            let projection_ty = relation.relate(a_data, b_data)?;
            Ok(tcx.mk_projection(projection_ty.item_def_id, projection_ty.substs))
        }

        (&ty::Opaque(a_def_id, a_substs), &ty::Opaque(b_def_id, b_substs))
            if a_def_id == b_def_id =>
        {
            let substs = relate_substs(relation, None, a_substs, b_substs)?;
            Ok(tcx.mk_opaque(a_def_id, substs))
        }

        _ => Err(TypeError::Sorts(expected_found(relation, &a, &b))),
    }
}

/// The main "const relation" routine. Note that this does not handle
/// inference artifacts, so you should filter those out before calling
/// it.
pub fn super_relate_consts<R: TypeRelation<'tcx>>(
    relation: &mut R,
    a: &'tcx ty::Const<'tcx>,
    b: &'tcx ty::Const<'tcx>,
) -> RelateResult<'tcx, &'tcx ty::Const<'tcx>> {
    debug!("{}.super_relate_consts(a = {:?}, b = {:?})", relation.tag(), a, b);
    let tcx = relation.tcx();

    let eagerly_eval = |x: &'tcx ty::Const<'tcx>| {
        // FIXME(eddyb) this doesn't account for lifetime inference variables
        // being erased by `eval`, *nor* for the polymorphic aspect of `eval`.
        // That is, we could always use `eval` and it will just return the
        // old value back if it doesn't succeed.
        if !x.val.needs_infer() {
            return x.eval(tcx, relation.param_env()).val;
        }
        x.val
    };

    // FIXME(eddyb) doesn't look like everything below checks that `a.ty == b.ty`.
    // We could probably always assert it early, as `const` generic parameters
    // are not allowed to depend on other generic parameters, i.e. are concrete.
    // (although there could be normalization differences)

    // Currently, the values that can be unified are primitive types,
    // and those that derive both `PartialEq` and `Eq`, corresponding
    // to structural-match types.
    let new_const_val = match (eagerly_eval(a), eagerly_eval(b)) {
        (ty::ConstKind::Infer(_), _) | (_, ty::ConstKind::Infer(_)) => {
            // The caller should handle these cases!
            bug!("var types encountered in super_relate_consts: {:?} {:?}", a, b)
        }

        (ty::ConstKind::Error, _) | (_, ty::ConstKind::Error) => Ok(ty::ConstKind::Error),

        (ty::ConstKind::Param(a_p), ty::ConstKind::Param(b_p)) if a_p.index == b_p.index => {
            return Ok(a);
        }
        (ty::ConstKind::Placeholder(p1), ty::ConstKind::Placeholder(p2)) if p1 == p2 => {
            return Ok(a);
        }
        (ty::ConstKind::Value(a_val), ty::ConstKind::Value(b_val)) => {
            let new_val = match (a_val, b_val) {
                (ConstValue::Scalar(a_val), ConstValue::Scalar(b_val)) if a.ty == b.ty => {
                    if a_val == b_val {
                        Ok(ConstValue::Scalar(a_val))
                    } else if let ty::FnPtr(_) = a.ty.kind {
                        let a_instance = tcx.global_alloc(a_val.assert_ptr().alloc_id).unwrap_fn();
                        let b_instance = tcx.global_alloc(b_val.assert_ptr().alloc_id).unwrap_fn();
                        if a_instance == b_instance {
                            Ok(ConstValue::Scalar(a_val))
                        } else {
                            Err(TypeError::ConstMismatch(expected_found(relation, &a, &b)))
                        }
                    } else {
                        Err(TypeError::ConstMismatch(expected_found(relation, &a, &b)))
                    }
                }

                (ConstValue::Slice { .. }, ConstValue::Slice { .. }) => {
                    let a_bytes = get_slice_bytes(&tcx, a_val);
                    let b_bytes = get_slice_bytes(&tcx, b_val);
                    if a_bytes == b_bytes {
                        Ok(a_val)
                    } else {
                        Err(TypeError::ConstMismatch(expected_found(relation, &a, &b)))
                    }
                }

                (ConstValue::ByRef { .. }, ConstValue::ByRef { .. }) => {
                    match a.ty.kind {
                        ty::Array(..) | ty::Adt(..) | ty::Tuple(..) => {
                            let a_destructured = tcx.destructure_const(relation.param_env().and(a));
                            let b_destructured = tcx.destructure_const(relation.param_env().and(b));

                            // Both the variant and each field have to be equal.
                            if a_destructured.variant == b_destructured.variant {
                                for (a_field, b_field) in
                                    a_destructured.fields.iter().zip(b_destructured.fields.iter())
                                {
                                    relation.consts(a_field, b_field)?;
                                }

                                Ok(a_val)
                            } else {
                                Err(TypeError::ConstMismatch(expected_found(relation, &a, &b)))
                            }
                        }
                        // FIXME(const_generics): There are probably some `TyKind`s
                        // which should be handled here.
                        _ => {
                            tcx.sess.delay_span_bug(
                                DUMMY_SP,
                                &format!("unexpected consts: a: {:?}, b: {:?}", a, b),
                            );
                            Err(TypeError::ConstMismatch(expected_found(relation, &a, &b)))
                        }
                    }
                }

                _ => Err(TypeError::ConstMismatch(expected_found(relation, &a, &b))),
            };

            new_val.map(ty::ConstKind::Value)
        }

        // FIXME(const_generics): this is wrong, as it is a projection
        (
            ty::ConstKind::Unevaluated(a_def_id, a_substs, a_promoted),
            ty::ConstKind::Unevaluated(b_def_id, b_substs, b_promoted),
        ) if a_def_id == b_def_id && a_promoted == b_promoted => {
            let substs =
                relation.relate_with_variance(ty::Variance::Invariant, &a_substs, &b_substs)?;
            Ok(ty::ConstKind::Unevaluated(a_def_id, &substs, a_promoted))
        }
        _ => Err(TypeError::ConstMismatch(expected_found(relation, &a, &b))),
    };
    new_const_val.map(|val| tcx.mk_const(ty::Const { val, ty: a.ty }))
}

impl<'tcx> Relate<'tcx> for &'tcx ty::List<ty::ExistentialPredicate<'tcx>> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Self,
        b: &Self,
    ) -> RelateResult<'tcx, Self> {
        if a.len() != b.len() {
            return Err(TypeError::ExistentialMismatch(expected_found(relation, a, b)));
        }

        let tcx = relation.tcx();
        let v = a.iter().zip(b.iter()).map(|(ep_a, ep_b)| {
            use crate::ty::ExistentialPredicate::*;
            match (*ep_a, *ep_b) {
                (Trait(ref a), Trait(ref b)) => Ok(Trait(relation.relate(a, b)?)),
                (Projection(ref a), Projection(ref b)) => Ok(Projection(relation.relate(a, b)?)),
                (AutoTrait(ref a), AutoTrait(ref b)) if a == b => Ok(AutoTrait(*a)),
                _ => Err(TypeError::ExistentialMismatch(expected_found(relation, a, b))),
            }
        });
        Ok(tcx.mk_existential_predicates(v)?)
    }
}

impl<'tcx> Relate<'tcx> for ty::ClosureSubsts<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::ClosureSubsts<'tcx>,
        b: &ty::ClosureSubsts<'tcx>,
    ) -> RelateResult<'tcx, ty::ClosureSubsts<'tcx>> {
        let substs = relate_substs(relation, None, a.substs, b.substs)?;
        Ok(ty::ClosureSubsts { substs })
    }
}

impl<'tcx> Relate<'tcx> for ty::GeneratorSubsts<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::GeneratorSubsts<'tcx>,
        b: &ty::GeneratorSubsts<'tcx>,
    ) -> RelateResult<'tcx, ty::GeneratorSubsts<'tcx>> {
        let substs = relate_substs(relation, None, a.substs, b.substs)?;
        Ok(ty::GeneratorSubsts { substs })
    }
}

impl<'tcx> Relate<'tcx> for SubstsRef<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &SubstsRef<'tcx>,
        b: &SubstsRef<'tcx>,
    ) -> RelateResult<'tcx, SubstsRef<'tcx>> {
        relate_substs(relation, None, a, b)
    }
}

impl<'tcx> Relate<'tcx> for ty::Region<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::Region<'tcx>,
        b: &ty::Region<'tcx>,
    ) -> RelateResult<'tcx, ty::Region<'tcx>> {
        relation.regions(*a, *b)
    }
}

impl<'tcx> Relate<'tcx> for &'tcx ty::Const<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &&'tcx ty::Const<'tcx>,
        b: &&'tcx ty::Const<'tcx>,
    ) -> RelateResult<'tcx, &'tcx ty::Const<'tcx>> {
        relation.consts(*a, *b)
    }
}

impl<'tcx, T: Relate<'tcx>> Relate<'tcx> for ty::Binder<T> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::Binder<T>,
        b: &ty::Binder<T>,
    ) -> RelateResult<'tcx, ty::Binder<T>> {
        relation.binders(a, b)
    }
}

impl<'tcx, T: Relate<'tcx>> Relate<'tcx> for Rc<T> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Rc<T>,
        b: &Rc<T>,
    ) -> RelateResult<'tcx, Rc<T>> {
        let a: &T = a;
        let b: &T = b;
        Ok(Rc::new(relation.relate(a, b)?))
    }
}

impl<'tcx, T: Relate<'tcx>> Relate<'tcx> for Box<T> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &Box<T>,
        b: &Box<T>,
    ) -> RelateResult<'tcx, Box<T>> {
        let a: &T = a;
        let b: &T = b;
        Ok(Box::new(relation.relate(a, b)?))
    }
}

impl<'tcx> Relate<'tcx> for GenericArg<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &GenericArg<'tcx>,
        b: &GenericArg<'tcx>,
    ) -> RelateResult<'tcx, GenericArg<'tcx>> {
        match (a.unpack(), b.unpack()) {
            (GenericArgKind::Lifetime(a_lt), GenericArgKind::Lifetime(b_lt)) => {
                Ok(relation.relate(&a_lt, &b_lt)?.into())
            }
            (GenericArgKind::Type(a_ty), GenericArgKind::Type(b_ty)) => {
                Ok(relation.relate(&a_ty, &b_ty)?.into())
            }
            (GenericArgKind::Const(a_ct), GenericArgKind::Const(b_ct)) => {
                Ok(relation.relate(&a_ct, &b_ct)?.into())
            }
            (GenericArgKind::Lifetime(unpacked), x) => {
                bug!("impossible case reached: can't relate: {:?} with {:?}", unpacked, x)
            }
            (GenericArgKind::Type(unpacked), x) => {
                bug!("impossible case reached: can't relate: {:?} with {:?}", unpacked, x)
            }
            (GenericArgKind::Const(unpacked), x) => {
                bug!("impossible case reached: can't relate: {:?} with {:?}", unpacked, x)
            }
        }
    }
}

impl<'tcx> Relate<'tcx> for ty::TraitPredicate<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::TraitPredicate<'tcx>,
        b: &ty::TraitPredicate<'tcx>,
    ) -> RelateResult<'tcx, ty::TraitPredicate<'tcx>> {
        Ok(ty::TraitPredicate { trait_ref: relation.relate(&a.trait_ref, &b.trait_ref)? })
    }
}

impl<'tcx> Relate<'tcx> for ty::ProjectionPredicate<'tcx> {
    fn relate<R: TypeRelation<'tcx>>(
        relation: &mut R,
        a: &ty::ProjectionPredicate<'tcx>,
        b: &ty::ProjectionPredicate<'tcx>,
    ) -> RelateResult<'tcx, ty::ProjectionPredicate<'tcx>> {
        Ok(ty::ProjectionPredicate {
            projection_ty: relation.relate(&a.projection_ty, &b.projection_ty)?,
            ty: relation.relate(&a.ty, &b.ty)?,
        })
    }
}

///////////////////////////////////////////////////////////////////////////
// Error handling

pub fn expected_found<R, T>(relation: &mut R, a: &T, b: &T) -> ExpectedFound<T>
where
    R: TypeRelation<'tcx>,
    T: Clone,
{
    expected_found_bool(relation.a_is_expected(), a, b)
}

pub fn expected_found_bool<T>(a_is_expected: bool, a: &T, b: &T) -> ExpectedFound<T>
where
    T: Clone,
{
    let a = a.clone();
    let b = b.clone();
    if a_is_expected {
        ExpectedFound { expected: a, found: b }
    } else {
        ExpectedFound { expected: b, found: a }
    }
}
