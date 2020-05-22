//! **Canonicalization** is the key to constructing a query in the
//! middle of type inference. Ordinarily, it is not possible to store
//! types from type inference in query keys, because they contain
//! references to inference variables whose lifetimes are too short
//! and so forth. Canonicalizing a value T1 using `canonicalize_query`
//! produces two things:
//!
//! - a value T2 where each unbound inference variable has been
//!   replaced with a **canonical variable**;
//! - a map M (of type `CanonicalVarValues`) from those canonical
//!   variables back to the original.
//!
//! We can then do queries using T2. These will give back constraints
//! on the canonical variables which can be translated, using the map
//! M, into constraints in our source context. This process of
//! translating the results back is done by the
//! `instantiate_query_result` method.
//!
//! For a more detailed look at what is happening here, check
//! out the [chapter in the rustc dev guide][c].
//!
//! [c]: https://rust-lang.github.io/chalk/book/canonical_queries/canonicalization.html

use crate::infer::{ConstVariableOrigin, ConstVariableOriginKind};
use crate::infer::{InferCtxt, RegionVariableOrigin, TypeVariableOrigin, TypeVariableOriginKind};
use rustc_index::vec::IndexVec;
use rustc_middle::ty::fold::TypeFoldable;
use rustc_middle::ty::subst::GenericArg;
use rustc_middle::ty::{self, BoundVar, List};
use rustc_span::source_map::Span;

pub use rustc_middle::infer::canonical::*;
use substitute::CanonicalExt;

mod canonicalizer;
pub mod query_response;
mod substitute;

impl<'cx, 'tcx> InferCtxt<'cx, 'tcx> {
    /// Creates a substitution S for the canonical value with fresh
    /// inference variables and applies it to the canonical value.
    /// Returns both the instantiated result *and* the substitution S.
    ///
    /// This is only meant to be invoked as part of constructing an
    /// inference context at the start of a query (see
    /// `InferCtxtBuilder::enter_with_canonical`). It basically
    /// brings the canonical value "into scope" within your new infcx.
    ///
    /// At the end of processing, the substitution S (once
    /// canonicalized) then represents the values that you computed
    /// for each of the canonical inputs to your query.

    pub fn instantiate_canonical_with_fresh_inference_vars<T>(
        &self,
        span: Span,
        canonical: &Canonical<'tcx, T>,
    ) -> (T, CanonicalVarValues<'tcx>)
    where
        T: TypeFoldable<'tcx>,
    {
        // For each universe that is referred to in the incoming
        // query, create a universe in our local inference context. In
        // practice, as of this writing, all queries have no universes
        // in them, so this code has no effect, but it is looking
        // forward to the day when we *do* want to carry universes
        // through into queries.
        let universes: IndexVec<ty::UniverseIndex, _> = std::iter::once(ty::UniverseIndex::ROOT)
            .chain((0..canonical.max_universe.as_u32()).map(|_| self.create_next_universe()))
            .collect();

        let canonical_inference_vars =
            self.instantiate_canonical_vars(span, canonical.variables, |ui| universes[ui]);
        let result = canonical.substitute(self.tcx, &canonical_inference_vars);
        (result, canonical_inference_vars)
    }

    /// Given the "infos" about the canonical variables from some
    /// canonical, creates fresh variables with the same
    /// characteristics (see `instantiate_canonical_var` for
    /// details). You can then use `substitute` to instantiate the
    /// canonical variable with these inference variables.
    fn instantiate_canonical_vars(
        &self,
        span: Span,
        variables: &List<CanonicalVarInfo>,
        universe_map: impl Fn(ty::UniverseIndex) -> ty::UniverseIndex,
    ) -> CanonicalVarValues<'tcx> {
        let var_values: IndexVec<BoundVar, GenericArg<'tcx>> = variables
            .iter()
            .map(|info| self.instantiate_canonical_var(span, *info, &universe_map))
            .collect();

        CanonicalVarValues { var_values }
    }

    /// Given the "info" about a canonical variable, creates a fresh
    /// variable for it. If this is an existentially quantified
    /// variable, then you'll get a new inference variable; if it is a
    /// universally quantified variable, you get a placeholder.
    fn instantiate_canonical_var(
        &self,
        span: Span,
        cv_info: CanonicalVarInfo,
        universe_map: impl Fn(ty::UniverseIndex) -> ty::UniverseIndex,
    ) -> GenericArg<'tcx> {
        match cv_info.kind {
            CanonicalVarKind::Ty(ty_kind) => {
                let ty = match ty_kind {
                    CanonicalTyVarKind::General(ui) => self.next_ty_var_in_universe(
                        TypeVariableOrigin { kind: TypeVariableOriginKind::MiscVariable, span },
                        universe_map(ui),
                    ),

                    CanonicalTyVarKind::Int => self.next_int_var(),

                    CanonicalTyVarKind::Float => self.next_float_var(),
                };
                ty.into()
            }

            CanonicalVarKind::PlaceholderTy(ty::PlaceholderType { universe, name }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderType { universe: universe_mapped, name };
                self.tcx.mk_ty(ty::Placeholder(placeholder_mapped)).into()
            }

            CanonicalVarKind::Region(ui) => self
                .next_region_var_in_universe(
                    RegionVariableOrigin::MiscVariable(span),
                    universe_map(ui),
                )
                .into(),

            CanonicalVarKind::PlaceholderRegion(ty::PlaceholderRegion { universe, name }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderRegion { universe: universe_mapped, name };
                self.tcx.mk_region(ty::RePlaceholder(placeholder_mapped)).into()
            }

            CanonicalVarKind::Const(ui) => self
                .next_const_var_in_universe(
                    self.next_ty_var_in_universe(
                        TypeVariableOrigin { kind: TypeVariableOriginKind::MiscVariable, span },
                        universe_map(ui),
                    ),
                    ConstVariableOrigin { kind: ConstVariableOriginKind::MiscVariable, span },
                    universe_map(ui),
                )
                .into(),

            CanonicalVarKind::PlaceholderConst(ty::PlaceholderConst { universe, name }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderConst { universe: universe_mapped, name };
                self.tcx
                    .mk_const(ty::Const {
                        val: ty::ConstKind::Placeholder(placeholder_mapped),
                        ty: self.tcx.types.err, // FIXME(const_generics)
                    })
                    .into()
            }
        }
    }
}
