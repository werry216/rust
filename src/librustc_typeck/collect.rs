// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! "Collection" is the process of determining the type and other external
//! details of each item in Rust. Collection is specifically concerned
//! with *interprocedural* things -- for example, for a function
//! definition, collection will figure out the type and signature of the
//! function, but it will not visit the *body* of the function in any way,
//! nor examine type annotations on local variables (that's the job of
//! type *checking*).
//!
//! Collecting is ultimately defined by a bundle of queries that
//! inquire after various facts about the items in the crate (e.g.,
//! `type_of`, `generics_of`, `predicates_of`, etc). See the `provide` function
//! for the full set.
//!
//! At present, however, we do run collection across all items in the
//! crate as a kind of pass. This should eventually be factored away.

use astconv::{AstConv, Bounds};
use lint;
use constrained_type_params as ctp;
use middle::lang_items::SizedTraitLangItem;
use middle::resolve_lifetime as rl;
use rustc::mir::mono::Linkage;
use rustc::ty::subst::Substs;
use rustc::ty::{ToPredicate, ReprOptions};
use rustc::ty::{self, AdtKind, ToPolyTraitRef, Ty, TyCtxt};
use rustc::ty::query::Providers;
use rustc::ty::util::IntTypeExt;
use rustc::ty::util::Discr;
use rustc::util::captures::Captures;
use rustc::util::nodemap::FxHashMap;
use rustc_target::spec::abi;

use syntax::ast;
use syntax::ast::MetaItemKind;
use syntax::attr::{InlineAttr, list_contains_name, mark_used};
use syntax::codemap::Spanned;
use syntax::symbol::{Symbol, keywords};
use syntax::feature_gate;
use syntax_pos::{Span, DUMMY_SP};

use rustc::hir::{self, map as hir_map, CodegenFnAttrs, CodegenFnAttrFlags, Unsafety};
use rustc::hir::GenericParamKind;
use rustc::hir::intravisit::{self, Visitor, NestedVisitorMap};
use rustc::hir::def::{Def, CtorKind};
use rustc::hir::def_id::{DefId, LOCAL_CRATE};

///////////////////////////////////////////////////////////////////////////
// Main entry point

pub fn collect_item_types<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>) {
    let mut visitor = CollectItemTypesVisitor { tcx: tcx };
    tcx.hir.krate().visit_all_item_likes(&mut visitor.as_deep_visitor());
}

pub fn provide(providers: &mut Providers) {
    *providers = Providers {
        type_of,
        generics_of,
        predicates_of,
        explicit_predicates_of,
        super_predicates_of,
        type_param_predicates,
        trait_def,
        adt_def,
        fn_sig,
        impl_trait_ref,
        impl_polarity,
        is_foreign_item,
        codegen_fn_attrs,
        ..*providers
    };
}

///////////////////////////////////////////////////////////////////////////

/// Context specific to some particular item. This is what implements
/// AstConv. It has information about the predicates that are defined
/// on the trait. Unfortunately, this predicate information is
/// available in various different forms at various points in the
/// process. So we can't just store a pointer to e.g. the AST or the
/// parsed ty form, we have to be more flexible. To this end, the
/// `ItemCtxt` is parameterized by a `DefId` that it uses to satisfy
/// `get_type_parameter_bounds` requests, drawing the information from
/// the AST (`hir::Generics`), recursively.
pub struct ItemCtxt<'a,'tcx:'a> {
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    item_def_id: DefId,
}

///////////////////////////////////////////////////////////////////////////

struct CollectItemTypesVisitor<'a, 'tcx: 'a> {
    tcx: TyCtxt<'a, 'tcx, 'tcx>
}

impl<'a, 'tcx> Visitor<'tcx> for CollectItemTypesVisitor<'a, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'tcx> {
        NestedVisitorMap::OnlyBodies(&self.tcx.hir)
    }

    fn visit_item(&mut self, item: &'tcx hir::Item) {
        convert_item(self.tcx, item.id);
        intravisit::walk_item(self, item);
    }

    fn visit_generics(&mut self, generics: &'tcx hir::Generics) {
        for param in &generics.params {
            match param.kind {
                hir::GenericParamKind::Lifetime { .. } => {}
                hir::GenericParamKind::Type { ref default, .. } if default.is_some() => {
                    let def_id = self.tcx.hir.local_def_id(param.id);
                    self.tcx.type_of(def_id);
                }
                hir::GenericParamKind::Type { .. } => {}
            }
        }
        intravisit::walk_generics(self, generics);
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr) {
        if let hir::ExprClosure(..) = expr.node {
            let def_id = self.tcx.hir.local_def_id(expr.id);
            self.tcx.generics_of(def_id);
            self.tcx.type_of(def_id);
        }
        intravisit::walk_expr(self, expr);
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem) {
        convert_trait_item(self.tcx, trait_item.id);
        intravisit::walk_trait_item(self, trait_item);
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        convert_impl_item(self.tcx, impl_item.id);
        intravisit::walk_impl_item(self, impl_item);
    }
}

///////////////////////////////////////////////////////////////////////////
// Utility types and common code for the above passes.

impl<'a, 'tcx> ItemCtxt<'a, 'tcx> {
    pub fn new(tcx: TyCtxt<'a, 'tcx, 'tcx>, item_def_id: DefId)
           -> ItemCtxt<'a,'tcx> {
        ItemCtxt {
            tcx,
            item_def_id,
        }
    }
}

impl<'a,'tcx> ItemCtxt<'a,'tcx> {
    pub fn to_ty(&self, ast_ty: &hir::Ty) -> Ty<'tcx> {
        AstConv::ast_ty_to_ty(self, ast_ty)
    }
}

impl<'a, 'tcx> AstConv<'tcx, 'tcx> for ItemCtxt<'a, 'tcx> {
    fn tcx<'b>(&'b self) -> TyCtxt<'b, 'tcx, 'tcx> { self.tcx }

    fn get_type_parameter_bounds(&self,
                                 span: Span,
                                 def_id: DefId)
                                 -> ty::GenericPredicates<'tcx>
    {
        self.tcx.at(span).type_param_predicates((self.item_def_id, def_id))
    }

    fn re_infer(&self, _span: Span, _def: Option<&ty::GenericParamDef>)
                -> Option<ty::Region<'tcx>> {
        None
    }

    fn ty_infer(&self, span: Span) -> Ty<'tcx> {
        struct_span_err!(
            self.tcx().sess,
            span,
            E0121,
            "the type placeholder `_` is not allowed within types on item signatures"
        ).span_label(span, "not allowed in type signatures")
        .emit();
        self.tcx().types.err
    }

    fn projected_ty_from_poly_trait_ref(&self,
                                        span: Span,
                                        item_def_id: DefId,
                                        poly_trait_ref: ty::PolyTraitRef<'tcx>)
                                        -> Ty<'tcx>
    {
        if let Some(trait_ref) = poly_trait_ref.no_late_bound_regions() {
            self.tcx().mk_projection(item_def_id, trait_ref.substs)
        } else {
            // no late-bound regions, we can just ignore the binder
            span_err!(self.tcx().sess, span, E0212,
                "cannot extract an associated type from a higher-ranked trait bound \
                 in this context");
            self.tcx().types.err
        }
    }

    fn normalize_ty(&self, _span: Span, ty: Ty<'tcx>) -> Ty<'tcx> {
        // types in item signatures are not normalized, to avoid undue
        // dependencies.
        ty
    }

    fn set_tainted_by_errors(&self) {
        // no obvious place to track this, just let it go
    }

    fn record_ty(&self, _hir_id: hir::HirId, _ty: Ty<'tcx>, _span: Span) {
        // no place to record types from signatures?
    }
}

fn type_param_predicates<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                   (item_def_id, def_id): (DefId, DefId))
                                   -> ty::GenericPredicates<'tcx> {
    use rustc::hir::map::*;
    use rustc::hir::*;

    // In the AST, bounds can derive from two places. Either
    // written inline like `<T:Foo>` or in a where clause like
    // `where T:Foo`.

    let param_id = tcx.hir.as_local_node_id(def_id).unwrap();
    let param_owner = tcx.hir.ty_param_owner(param_id);
    let param_owner_def_id = tcx.hir.local_def_id(param_owner);
    let generics = tcx.generics_of(param_owner_def_id);
    let index = generics.param_def_id_to_index[&def_id];
    let ty = tcx.mk_ty_param(index, tcx.hir.ty_param_name(param_id).as_interned_str());

    // Don't look for bounds where the type parameter isn't in scope.
    let parent = if item_def_id == param_owner_def_id {
        None
    } else {
        tcx.generics_of(item_def_id).parent
    };

    let mut result = parent.map_or(ty::GenericPredicates {
        parent: None,
        predicates: vec![]
    }, |parent| {
        let icx = ItemCtxt::new(tcx, parent);
        icx.get_type_parameter_bounds(DUMMY_SP, def_id)
    });

    let item_node_id = tcx.hir.as_local_node_id(item_def_id).unwrap();
    let ast_generics = match tcx.hir.get(item_node_id) {
        NodeTraitItem(item) => &item.generics,

        NodeImplItem(item) => &item.generics,

        NodeItem(item) => {
            match item.node {
                ItemFn(.., ref generics, _) |
                ItemImpl(_, _, _, ref generics, ..) |
                ItemTy(_, ref generics) |
                ItemEnum(_, ref generics) |
                ItemStruct(_, ref generics) |
                ItemUnion(_, ref generics) => generics,
                ItemTrait(_, _, ref generics, ..) => {
                    // Implied `Self: Trait` and supertrait bounds.
                    if param_id == item_node_id {
                        result.predicates.push(ty::TraitRef {
                            def_id: item_def_id,
                            substs: Substs::identity_for_item(tcx, item_def_id)
                        }.to_predicate());
                    }
                    generics
                }
                _ => return result
            }
        }

        NodeForeignItem(item) => {
            match item.node {
                ForeignItemFn(_, _, ref generics) => generics,
                _ => return result
            }
        }

        _ => return result
    };

    let icx = ItemCtxt::new(tcx, item_def_id);
    result.predicates.extend(
        icx.type_parameter_bounds_in_generics(ast_generics, param_id, ty));
    result
}

impl<'a, 'tcx> ItemCtxt<'a, 'tcx> {
    /// Find bounds from hir::Generics. This requires scanning through the
    /// AST. We do this to avoid having to convert *all* the bounds, which
    /// would create artificial cycles. Instead we can only convert the
    /// bounds for a type parameter `X` if `X::Foo` is used.
    fn type_parameter_bounds_in_generics(&self,
                                         ast_generics: &hir::Generics,
                                         param_id: ast::NodeId,
                                         ty: Ty<'tcx>)
                                         -> Vec<ty::Predicate<'tcx>>
    {
        let from_ty_params =
            ast_generics.params.iter()
                .filter_map(|param| match param.kind {
                    GenericParamKind::Type { .. } if param.id == param_id => Some(&param.bounds),
                    _ => None
                })
                .flat_map(|bounds| bounds.iter())
                .flat_map(|b| predicates_from_bound(self, ty, b));

        let from_where_clauses =
            ast_generics.where_clause
                .predicates
                .iter()
                .filter_map(|wp| match *wp {
                    hir::WherePredicate::BoundPredicate(ref bp) => Some(bp),
                    _ => None
                })
                .filter(|bp| is_param(self.tcx, &bp.bounded_ty, param_id))
                .flat_map(|bp| bp.bounds.iter())
                .flat_map(|b| predicates_from_bound(self, ty, b));

        from_ty_params.chain(from_where_clauses).collect()
    }
}

/// Tests whether this is the AST for a reference to the type
/// parameter with id `param_id`. We use this so as to avoid running
/// `ast_ty_to_ty`, because we want to avoid triggering an all-out
/// conversion of the type to avoid inducing unnecessary cycles.
fn is_param<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                      ast_ty: &hir::Ty,
                      param_id: ast::NodeId)
                      -> bool
{
    if let hir::TyPath(hir::QPath::Resolved(None, ref path)) = ast_ty.node {
        match path.def {
            Def::SelfTy(Some(def_id), None) |
            Def::TyParam(def_id) => {
                def_id == tcx.hir.local_def_id(param_id)
            }
            _ => false
        }
    } else {
        false
    }
}

fn convert_item<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, item_id: ast::NodeId) {
    let it = tcx.hir.expect_item(item_id);
    debug!("convert: item {} with id {}", it.name, it.id);
    let def_id = tcx.hir.local_def_id(item_id);
    match it.node {
        // These don't define types.
        hir::ItemExternCrate(_) |
        hir::ItemUse(..) |
        hir::ItemMod(_) |
        hir::ItemGlobalAsm(_) => {}
        hir::ItemForeignMod(ref foreign_mod) => {
            for item in &foreign_mod.items {
                let def_id = tcx.hir.local_def_id(item.id);
                tcx.generics_of(def_id);
                tcx.type_of(def_id);
                tcx.predicates_of(def_id);
                if let hir::ForeignItemFn(..) = item.node {
                    tcx.fn_sig(def_id);
                }
            }
        }
        hir::ItemEnum(ref enum_definition, _) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
            convert_enum_variant_types(tcx, def_id, &enum_definition.variants);
        },
        hir::ItemImpl(..) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.impl_trait_ref(def_id);
            tcx.predicates_of(def_id);
        },
        hir::ItemTrait(..) => {
            tcx.generics_of(def_id);
            tcx.trait_def(def_id);
            tcx.at(it.span).super_predicates_of(def_id);
            tcx.predicates_of(def_id);
        },
        hir::ItemTraitAlias(..) => {
            span_err!(tcx.sess, it.span, E0645,
                      "trait aliases are not yet implemented (see issue #41517)");
        },
        hir::ItemStruct(ref struct_def, _) |
        hir::ItemUnion(ref struct_def, _) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);

            for f in struct_def.fields() {
                let def_id = tcx.hir.local_def_id(f.id);
                tcx.generics_of(def_id);
                tcx.type_of(def_id);
                tcx.predicates_of(def_id);
            }

            if !struct_def.is_struct() {
                convert_variant_ctor(tcx, struct_def.id());
            }
        },
        hir::ItemExistential(..) |
        hir::ItemTy(..) | hir::ItemStatic(..) | hir::ItemConst(..) | hir::ItemFn(..) => {
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
            if let hir::ItemFn(..) = it.node {
                tcx.fn_sig(def_id);
            }
        }
    }
}

fn convert_trait_item<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, trait_item_id: ast::NodeId) {
    let trait_item = tcx.hir.expect_trait_item(trait_item_id);
    let def_id = tcx.hir.local_def_id(trait_item.id);
    tcx.generics_of(def_id);

    match trait_item.node {
        hir::TraitItemKind::Const(..) |
        hir::TraitItemKind::Type(_, Some(_)) |
        hir::TraitItemKind::Method(..) => {
            tcx.type_of(def_id);
            if let hir::TraitItemKind::Method(..) = trait_item.node {
                tcx.fn_sig(def_id);
            }
        }

        hir::TraitItemKind::Type(_, None) => {}
    };

    tcx.predicates_of(def_id);
}

fn convert_impl_item<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, impl_item_id: ast::NodeId) {
    let def_id = tcx.hir.local_def_id(impl_item_id);
    tcx.generics_of(def_id);
    tcx.type_of(def_id);
    tcx.predicates_of(def_id);
    if let hir::ImplItemKind::Method(..) = tcx.hir.expect_impl_item(impl_item_id).node {
        tcx.fn_sig(def_id);
    }
}

fn convert_variant_ctor<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                  ctor_id: ast::NodeId) {
    let def_id = tcx.hir.local_def_id(ctor_id);
    tcx.generics_of(def_id);
    tcx.type_of(def_id);
    tcx.predicates_of(def_id);
}

fn convert_enum_variant_types<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                        def_id: DefId,
                                        variants: &[hir::Variant]) {
    let def = tcx.adt_def(def_id);
    let repr_type = def.repr.discr_type();
    let initial = repr_type.initial_discriminant(tcx);
    let mut prev_discr = None::<Discr<'tcx>>;

    // fill the discriminant values and field types
    for variant in variants {
        let wrapped_discr = prev_discr.map_or(initial, |d| d.wrap_incr(tcx));
        prev_discr = Some(if let Some(ref e) = variant.node.disr_expr {
            let expr_did = tcx.hir.local_def_id(e.id);
            def.eval_explicit_discr(tcx, expr_did)
        } else if let Some(discr) = repr_type.disr_incr(tcx, prev_discr) {
            Some(discr)
        } else {
            struct_span_err!(tcx.sess, variant.span, E0370,
                             "enum discriminant overflowed")
                .span_label(variant.span, format!("overflowed on value after {}",
                                                   prev_discr.unwrap()))
                .note(&format!("explicitly set `{} = {}` if that is desired outcome",
                               variant.node.name, wrapped_discr))
                .emit();
            None
        }.unwrap_or(wrapped_discr));

        for f in variant.node.data.fields() {
            let def_id = tcx.hir.local_def_id(f.id);
            tcx.generics_of(def_id);
            tcx.type_of(def_id);
            tcx.predicates_of(def_id);
        }

        // Convert the ctor, if any. This also registers the variant as
        // an item.
        convert_variant_ctor(tcx, variant.node.data.id());
    }
}

fn convert_struct_variant<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                    did: DefId,
                                    name: ast::Name,
                                    discr: ty::VariantDiscr,
                                    def: &hir::VariantData)
                                    -> ty::VariantDef {
    let mut seen_fields: FxHashMap<ast::Ident, Span> = FxHashMap();
    let node_id = tcx.hir.as_local_node_id(did).unwrap();
    let fields = def.fields().iter().map(|f| {
        let fid = tcx.hir.local_def_id(f.id);
        let dup_span = seen_fields.get(&f.ident.modern()).cloned();
        if let Some(prev_span) = dup_span {
            struct_span_err!(tcx.sess, f.span, E0124,
                             "field `{}` is already declared",
                             f.ident)
                .span_label(f.span, "field already declared")
                .span_label(prev_span, format!("`{}` first declared here", f.ident))
                .emit();
        } else {
            seen_fields.insert(f.ident.modern(), f.span);
        }

        ty::FieldDef {
            did: fid,
            ident: f.ident,
            vis: ty::Visibility::from_hir(&f.vis, node_id, tcx)
        }
    }).collect();
    ty::VariantDef {
        did,
        name,
        discr,
        fields,
        ctor_kind: CtorKind::from_hir(def),
    }
}

fn adt_def<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                     def_id: DefId)
                     -> &'tcx ty::AdtDef {
    use rustc::hir::map::*;
    use rustc::hir::*;

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();
    let item = match tcx.hir.get(node_id) {
        NodeItem(item) => item,
        _ => bug!()
    };

    let repr = ReprOptions::new(tcx, def_id);
    let (kind, variants) = match item.node {
        ItemEnum(ref def, _) => {
            let mut distance_from_explicit = 0;
            (AdtKind::Enum, def.variants.iter().map(|v| {
                let did = tcx.hir.local_def_id(v.node.data.id());
                let discr = if let Some(ref e) = v.node.disr_expr {
                    distance_from_explicit = 0;
                    ty::VariantDiscr::Explicit(tcx.hir.local_def_id(e.id))
                } else {
                    ty::VariantDiscr::Relative(distance_from_explicit)
                };
                distance_from_explicit += 1;

                convert_struct_variant(tcx, did, v.node.name, discr, &v.node.data)
            }).collect())
        }
        ItemStruct(ref def, _) => {
            // Use separate constructor id for unit/tuple structs and reuse did for braced structs.
            let ctor_id = if !def.is_struct() {
                Some(tcx.hir.local_def_id(def.id()))
            } else {
                None
            };
            (AdtKind::Struct, vec![
                convert_struct_variant(tcx, ctor_id.unwrap_or(def_id), item.name,
                                       ty::VariantDiscr::Relative(0), def)
            ])
        }
        ItemUnion(ref def, _) => {
            (AdtKind::Union, vec![
                convert_struct_variant(tcx, def_id, item.name,
                                       ty::VariantDiscr::Relative(0), def)
            ])
        }
        _ => bug!()
    };
    tcx.alloc_adt_def(def_id, kind, variants, repr)
}

/// Ensures that the super-predicates of the trait with def-id
/// trait_def_id are converted and stored. This also ensures that
/// the transitive super-predicates are converted;
fn super_predicates_of<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                 trait_def_id: DefId)
                                 -> ty::GenericPredicates<'tcx> {
    debug!("super_predicates(trait_def_id={:?})", trait_def_id);
    let trait_node_id = tcx.hir.as_local_node_id(trait_def_id).unwrap();

    let item = match tcx.hir.get(trait_node_id) {
        hir_map::NodeItem(item) => item,
        _ => bug!("trait_node_id {} is not an item", trait_node_id)
    };

    let (generics, bounds) = match item.node {
        hir::ItemTrait(.., ref generics, ref supertraits, _) => (generics, supertraits),
        hir::ItemTraitAlias(ref generics, ref supertraits) => (generics, supertraits),
        _ => span_bug!(item.span,
                       "super_predicates invoked on non-trait"),
    };

    let icx = ItemCtxt::new(tcx, trait_def_id);

    // Convert the bounds that follow the colon, e.g. `Bar+Zed` in `trait Foo : Bar+Zed`.
    let self_param_ty = tcx.mk_self_type();
    let superbounds1 = compute_bounds(&icx,
                                      self_param_ty,
                                      bounds,
                                      SizedByDefault::No,
                                      item.span);

    let superbounds1 = superbounds1.predicates(tcx, self_param_ty);

    // Convert any explicit superbounds in the where clause,
    // e.g. `trait Foo where Self : Bar`:
    let superbounds2 = icx.type_parameter_bounds_in_generics(generics, item.id, self_param_ty);

    // Combine the two lists to form the complete set of superbounds:
    let superbounds: Vec<_> = superbounds1.into_iter().chain(superbounds2).collect();

    // Now require that immediate supertraits are converted,
    // which will, in turn, reach indirect supertraits.
    for bound in superbounds.iter().filter_map(|p| p.to_opt_poly_trait_ref()) {
        tcx.at(item.span).super_predicates_of(bound.def_id());
    }

    ty::GenericPredicates {
        parent: None,
        predicates: superbounds
    }
}

fn trait_def<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                       def_id: DefId)
                       -> &'tcx ty::TraitDef {
    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();
    let item = tcx.hir.expect_item(node_id);

    let (is_auto, unsafety) = match item.node {
        hir::ItemTrait(is_auto, unsafety, ..) => (is_auto == hir::IsAuto::Yes, unsafety),
        hir::ItemTraitAlias(..) => (false, hir::Unsafety::Normal),
        _ => span_bug!(item.span, "trait_def_of_item invoked on non-trait"),
    };

    let paren_sugar = tcx.has_attr(def_id, "rustc_paren_sugar");
    if paren_sugar && !tcx.features().unboxed_closures {
        let mut err = tcx.sess.struct_span_err(
            item.span,
            "the `#[rustc_paren_sugar]` attribute is a temporary means of controlling \
             which traits can use parenthetical notation");
        help!(&mut err,
            "add `#![feature(unboxed_closures)]` to \
             the crate attributes to use it");
        err.emit();
    }

    let def_path_hash = tcx.def_path_hash(def_id);
    let def = ty::TraitDef::new(def_id,
                                unsafety,
                                paren_sugar,
                                is_auto,
                                def_path_hash);
    tcx.alloc_trait_def(def)
}

fn has_late_bound_regions<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                    node: hir_map::Node<'tcx>)
                                    -> Option<Span> {
    struct LateBoundRegionsDetector<'a, 'tcx: 'a> {
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        outer_index: ty::DebruijnIndex,
        has_late_bound_regions: Option<Span>,
    }

    impl<'a, 'tcx> Visitor<'tcx> for LateBoundRegionsDetector<'a, 'tcx> {
        fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'tcx> {
            NestedVisitorMap::None
        }

        fn visit_ty(&mut self, ty: &'tcx hir::Ty) {
            if self.has_late_bound_regions.is_some() { return }
            match ty.node {
                hir::TyBareFn(..) => {
                    self.outer_index.shift_in(1);
                    intravisit::walk_ty(self, ty);
                    self.outer_index.shift_out(1);
                }
                _ => intravisit::walk_ty(self, ty)
            }
        }

        fn visit_poly_trait_ref(&mut self,
                                tr: &'tcx hir::PolyTraitRef,
                                m: hir::TraitBoundModifier) {
            if self.has_late_bound_regions.is_some() { return }
            self.outer_index.shift_in(1);
            intravisit::walk_poly_trait_ref(self, tr, m);
            self.outer_index.shift_out(1);
        }

        fn visit_lifetime(&mut self, lt: &'tcx hir::Lifetime) {
            if self.has_late_bound_regions.is_some() { return }

            let hir_id = self.tcx.hir.node_to_hir_id(lt.id);
            match self.tcx.named_region(hir_id) {
                Some(rl::Region::Static) | Some(rl::Region::EarlyBound(..)) => {}
                Some(rl::Region::LateBound(debruijn, _, _)) |
                Some(rl::Region::LateBoundAnon(debruijn, _))
                    if debruijn < self.outer_index => {}
                Some(rl::Region::LateBound(..)) |
                Some(rl::Region::LateBoundAnon(..)) |
                Some(rl::Region::Free(..)) |
                None => {
                    self.has_late_bound_regions = Some(lt.span);
                }
            }
        }
    }

    fn has_late_bound_regions<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                        generics: &'tcx hir::Generics,
                                        decl: &'tcx hir::FnDecl)
                                        -> Option<Span> {
        let mut visitor = LateBoundRegionsDetector {
            tcx,
            outer_index: ty::INNERMOST,
            has_late_bound_regions: None,
        };
        for param in &generics.params {
            match param.kind {
                GenericParamKind::Lifetime { .. } => {
                    let hir_id = tcx.hir.node_to_hir_id(param.id);
                    if tcx.is_late_bound(hir_id) {
                        return Some(param.span);
                    }
                }
                _ => {},
            }
        }
        visitor.visit_fn_decl(decl);
        visitor.has_late_bound_regions
    }

    match node {
        hir_map::NodeTraitItem(item) => match item.node {
            hir::TraitItemKind::Method(ref sig, _) =>
                has_late_bound_regions(tcx, &item.generics, &sig.decl),
            _ => None,
        },
        hir_map::NodeImplItem(item) => match item.node {
            hir::ImplItemKind::Method(ref sig, _) =>
                has_late_bound_regions(tcx, &item.generics, &sig.decl),
            _ => None,
        },
        hir_map::NodeForeignItem(item) => match item.node {
            hir::ForeignItemFn(ref fn_decl, _, ref generics) =>
                has_late_bound_regions(tcx, generics, fn_decl),
            _ => None,
        },
        hir_map::NodeItem(item) => match item.node {
            hir::ItemFn(ref fn_decl, .., ref generics, _) =>
                has_late_bound_regions(tcx, generics, fn_decl),
            _ => None,
        },
        _ => None
    }
}

fn generics_of<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                         def_id: DefId)
                         -> &'tcx ty::Generics {
    use rustc::hir::map::*;
    use rustc::hir::*;

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();

    let node = tcx.hir.get(node_id);
    let parent_def_id = match node {
        NodeImplItem(_) |
        NodeTraitItem(_) |
        NodeVariant(_) |
        NodeStructCtor(_) |
        NodeField(_) => {
            let parent_id = tcx.hir.get_parent(node_id);
            Some(tcx.hir.local_def_id(parent_id))
        }
        NodeExpr(&hir::Expr { node: hir::ExprClosure(..), .. }) => {
            Some(tcx.closure_base_def_id(def_id))
        }
        NodeItem(item) => {
            match item.node {
                ItemExistential(hir::ExistTy { impl_trait_fn, .. }) => impl_trait_fn,
                _ => None,
            }
        },
        _ => None
    };

    let mut opt_self = None;
    let mut allow_defaults = false;

    let no_generics = hir::Generics::empty();
    let ast_generics = match node {
        NodeTraitItem(item) => &item.generics,

        NodeImplItem(item) => &item.generics,

        NodeItem(item) => {
            match item.node {
                ItemFn(.., ref generics, _) |
                ItemImpl(_, _, _, ref generics, ..) => generics,

                ItemTy(_, ref generics) |
                ItemEnum(_, ref generics) |
                ItemStruct(_, ref generics) |
                ItemExistential(hir::ExistTy { ref generics, .. }) |
                ItemUnion(_, ref generics) => {
                    allow_defaults = true;
                    generics
                }

                ItemTrait(_, _, ref generics, ..) | ItemTraitAlias(ref generics, ..) => {
                    // Add in the self type parameter.
                    //
                    // Something of a hack: use the node id for the trait, also as
                    // the node id for the Self type parameter.
                    let param_id = item.id;

                    opt_self = Some(ty::GenericParamDef {
                        index: 0,
                        name: keywords::SelfType.name().as_interned_str(),
                        def_id: tcx.hir.local_def_id(param_id),
                        pure_wrt_drop: false,
                        kind: ty::GenericParamDefKind::Type {
                            has_default: false,
                            object_lifetime_default: rl::Set1::Empty,
                            synthetic: None,
                        },
                    });

                    allow_defaults = true;
                    generics
                }

                _ => &no_generics,
            }
        }

        NodeForeignItem(item) => {
            match item.node {
                ForeignItemStatic(..) => &no_generics,
                ForeignItemFn(_, _, ref generics) => generics,
                ForeignItemType => &no_generics,
            }
        }

        NodeTy(&hir::Ty { node: hir::TyImplTraitExistential(..), .. }) => {
            bug!("impl Trait is desugared to existential type items");
        }

        _ => &no_generics,
    };

    let has_self = opt_self.is_some();
    let mut parent_has_self = false;
    let mut own_start = has_self as u32;
    let parent_count = parent_def_id.map_or(0, |def_id| {
        let generics = tcx.generics_of(def_id);
        assert_eq!(has_self, false);
        parent_has_self = generics.has_self;
        own_start = generics.count() as u32;
        generics.parent_count + generics.params.len()
    });

    let mut params: Vec<_> = opt_self.into_iter().collect();

    let early_lifetimes = early_bound_lifetimes_from_generics(tcx, ast_generics);
    params.extend(early_lifetimes.enumerate().map(|(i, param)| {
        ty::GenericParamDef {
            name: param.name.name().as_interned_str(),
            index: own_start + i as u32,
            def_id: tcx.hir.local_def_id(param.id),
            pure_wrt_drop: param.pure_wrt_drop,
            kind: ty::GenericParamDefKind::Lifetime,
        }
    }));

    let hir_id = tcx.hir.node_to_hir_id(node_id);
    let object_lifetime_defaults = tcx.object_lifetime_defaults(hir_id);

    // Now create the real type parameters.
    let type_start = own_start - has_self as u32 + params.len() as u32;
    let mut i = 0;
    params.extend(ast_generics.params.iter().filter_map(|param| match param.kind {
        GenericParamKind::Type { ref default, synthetic, .. } => {
            if param.name.name() == keywords::SelfType.name() {
                span_bug!(param.span,  "`Self` should not be the name of a regular parameter");
            }

            if !allow_defaults && default.is_some() {
                if !tcx.features().default_type_parameter_fallback {
                    tcx.lint_node(
                        lint::builtin::INVALID_TYPE_PARAM_DEFAULT,
                        param.id,
                        param.span,
                        &format!("defaults for type parameters are only allowed in \
                                    `struct`, `enum`, `type`, or `trait` definitions."));
                }
            }

            let ty_param = ty::GenericParamDef {
                index: type_start + i as u32,
                name: param.name.name().as_interned_str(),
                def_id: tcx.hir.local_def_id(param.id),
                pure_wrt_drop: param.pure_wrt_drop,
                kind: ty::GenericParamDefKind::Type {
                    has_default: default.is_some(),
                    object_lifetime_default:
                        object_lifetime_defaults.as_ref().map_or(rl::Set1::Empty, |o| o[i]),
                    synthetic,
                },
            };
            i += 1;
            Some(ty_param)
        }
        _ => None,
    }));

    // provide junk type parameter defs - the only place that
    // cares about anything but the length is instantiation,
    // and we don't do that for closures.
    if let NodeExpr(&hir::Expr { node: hir::ExprClosure(.., gen), .. }) = node {
        let dummy_args = if gen.is_some() {
            &["<yield_ty>", "<return_ty>", "<witness>"][..]
        } else {
            &["<closure_kind>", "<closure_signature>"][..]
        };

        for (i, &arg) in dummy_args.iter().enumerate() {
            params.push(ty::GenericParamDef {
                index: type_start + i as u32,
                name: Symbol::intern(arg).as_interned_str(),
                def_id,
                pure_wrt_drop: false,
                kind: ty::GenericParamDefKind::Type {
                    has_default: false,
                    object_lifetime_default: rl::Set1::Empty,
                    synthetic: None,
                },
            });
        }

        tcx.with_freevars(node_id, |fv| {
            params.extend(fv.iter().zip((dummy_args.len() as u32)..).map(|(_, i)| {
                ty::GenericParamDef {
                    index: type_start + i,
                    name: Symbol::intern("<upvar>").as_interned_str(),
                    def_id,
                    pure_wrt_drop: false,
                    kind: ty::GenericParamDefKind::Type {
                        has_default: false,
                        object_lifetime_default: rl::Set1::Empty,
                        synthetic: None,
                    },
                }
            }));
        });
    }

    let param_def_id_to_index = params.iter()
                                      .map(|param| (param.def_id, param.index))
                                      .collect();

    tcx.alloc_generics(ty::Generics {
        parent: parent_def_id,
        parent_count,
        params,
        param_def_id_to_index,
        has_self: has_self || parent_has_self,
        has_late_bound_regions: has_late_bound_regions(tcx, node),
    })
}

fn type_of<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                     def_id: DefId)
                     -> Ty<'tcx> {
    use rustc::hir::map::*;
    use rustc::hir::*;

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();

    let icx = ItemCtxt::new(tcx, def_id);

    match tcx.hir.get(node_id) {
        NodeTraitItem(item) => {
            match item.node {
                TraitItemKind::Method(..) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_fn_def(def_id, substs)
                }
                TraitItemKind::Const(ref ty, _) |
                TraitItemKind::Type(_, Some(ref ty)) => icx.to_ty(ty),
                TraitItemKind::Type(_, None) => {
                    span_bug!(item.span, "associated type missing default");
                }
            }
        }

        NodeImplItem(item) => {
            match item.node {
                ImplItemKind::Method(..) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_fn_def(def_id, substs)
                }
                ImplItemKind::Const(ref ty, _) => icx.to_ty(ty),
                ImplItemKind::Type(ref ty) => {
                    if tcx.impl_trait_ref(tcx.hir.get_parent_did(node_id)).is_none() {
                        span_err!(tcx.sess, item.span, E0202,
                                  "associated types are not allowed in inherent impls");
                    }

                    icx.to_ty(ty)
                }
            }
        }

        NodeItem(item) => {
            match item.node {
                ItemStatic(ref t, ..) | ItemConst(ref t, _) |
                ItemTy(ref t, _) | ItemImpl(.., ref t, _) => {
                    icx.to_ty(t)
                }
                ItemFn(..) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_fn_def(def_id, substs)
                }
                ItemEnum(..) |
                ItemStruct(..) |
                ItemUnion(..) => {
                    let def = tcx.adt_def(def_id);
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_adt(def, substs)
                }
                // this is only reachable once we have named existential types
                ItemExistential(hir::ExistTy { impl_trait_fn: None, .. }) => unimplemented!(),
                // existential types desugared from impl Trait
                ItemExistential(hir::ExistTy { impl_trait_fn: Some(owner), .. }) => {
                    tcx.typeck_tables_of(owner).concrete_existential_types[&def_id]
                },
                ItemTrait(..) | ItemTraitAlias(..) |
                ItemMod(..) |
                ItemForeignMod(..) |
                ItemGlobalAsm(..) |
                ItemExternCrate(..) |
                ItemUse(..) => {
                    span_bug!(
                        item.span,
                        "compute_type_of_item: unexpected item type: {:?}",
                        item.node);
                }
            }
        }

        NodeForeignItem(foreign_item) => {
            match foreign_item.node {
                ForeignItemFn(..) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_fn_def(def_id, substs)
                }
                ForeignItemStatic(ref t, _) => icx.to_ty(t),
                ForeignItemType => tcx.mk_foreign(def_id),
            }
        }

        NodeStructCtor(&ref def) |
        NodeVariant(&Spanned { node: hir::Variant_ { data: ref def, .. }, .. }) => {
            match *def {
                VariantData::Unit(..) | VariantData::Struct(..) => {
                    tcx.type_of(tcx.hir.get_parent_did(node_id))
                }
                VariantData::Tuple(..) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    tcx.mk_fn_def(def_id, substs)
                }
            }
        }

        NodeField(field) => icx.to_ty(&field.ty),

        NodeExpr(&hir::Expr { node: hir::ExprClosure(.., gen), .. }) => {
            if gen.is_some() {
                let hir_id = tcx.hir.node_to_hir_id(node_id);
                return tcx.typeck_tables_of(def_id).node_id_to_type(hir_id);
            }

            let substs = ty::ClosureSubsts {
                substs: Substs::identity_for_item(tcx, def_id),
            };

            tcx.mk_closure(def_id, substs)
        }

        NodeAnonConst(_) => match tcx.hir.get(tcx.hir.get_parent_node(node_id)) {
            NodeTy(&hir::Ty { node: TyArray(_, ref constant), .. }) |
            NodeTy(&hir::Ty { node: TyTypeof(ref constant), .. }) |
            NodeExpr(&hir::Expr { node: ExprRepeat(_, ref constant), .. })
                if constant.id == node_id => tcx.types.usize,

            NodeVariant(&Spanned { node: Variant_ { disr_expr: Some(ref e), .. }, .. })
                if e.id == node_id => {
                    tcx.adt_def(tcx.hir.get_parent_did(node_id))
                        .repr.discr_type().to_ty(tcx)
                }

            x => {
                bug!("unexpected const parent in type_of_def_id(): {:?}", x);
            }
        },

        NodeGenericParam(param) => {
            match param.kind {
                hir::GenericParamKind::Type { default: Some(ref ty), .. } => {
                    icx.to_ty(ty)
                }
                _ => bug!("unexpected non-type NodeGenericParam"),
            }
        }

        x => {
            bug!("unexpected sort of node in type_of_def_id(): {:?}", x);
        }
    }
}

fn fn_sig<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                    def_id: DefId)
                    -> ty::PolyFnSig<'tcx> {
    use rustc::hir::map::*;
    use rustc::hir::*;

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();

    let icx = ItemCtxt::new(tcx, def_id);

    match tcx.hir.get(node_id) {
        NodeTraitItem(&hir::TraitItem { node: TraitItemKind::Method(ref sig, _), .. }) |
        NodeImplItem(&hir::ImplItem { node: ImplItemKind::Method(ref sig, _), .. }) => {
            AstConv::ty_of_fn(&icx, sig.unsafety, sig.abi, &sig.decl)
        }

        NodeItem(&hir::Item { node: ItemFn(ref decl, unsafety, _, abi, _, _), .. }) => {
            AstConv::ty_of_fn(&icx, unsafety, abi, decl)
        }

        NodeForeignItem(&hir::ForeignItem { node: ForeignItemFn(ref fn_decl, _, _), .. }) => {
            let abi = tcx.hir.get_foreign_abi(node_id);
            compute_sig_of_foreign_fn_decl(tcx, def_id, fn_decl, abi)
        }

        NodeStructCtor(&VariantData::Tuple(ref fields, _)) |
        NodeVariant(&Spanned { node: hir::Variant_ {
            data: VariantData::Tuple(ref fields, _), ..
        }, .. }) => {
            let ty = tcx.type_of(tcx.hir.get_parent_did(node_id));
            let inputs = fields.iter().map(|f| {
                tcx.type_of(tcx.hir.local_def_id(f.id))
            });
            ty::Binder::bind(tcx.mk_fn_sig(
                inputs,
                ty,
                false,
                hir::Unsafety::Normal,
                abi::Abi::Rust
            ))
        }

        NodeExpr(&hir::Expr { node: hir::ExprClosure(..), .. }) => {
            // Closure signatures are not like other function
            // signatures and cannot be accessed through `fn_sig`. For
            // example, a closure signature excludes the `self`
            // argument. In any case they are embedded within the
            // closure type as part of the `ClosureSubsts`.
            //
            // To get
            // the signature of a closure, you should use the
            // `closure_sig` method on the `ClosureSubsts`:
            //
            //    closure_substs.closure_sig(def_id, tcx)
            //
            // or, inside of an inference context, you can use
            //
            //    infcx.closure_sig(def_id, closure_substs)
            bug!("to get the signature of a closure, use `closure_sig()` not `fn_sig()`");
        }

        x => {
            bug!("unexpected sort of node in fn_sig(): {:?}", x);
        }
    }
}

fn impl_trait_ref<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                            def_id: DefId)
                            -> Option<ty::TraitRef<'tcx>> {
    let icx = ItemCtxt::new(tcx, def_id);

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();
    match tcx.hir.expect_item(node_id).node {
        hir::ItemImpl(.., ref opt_trait_ref, _, _) => {
            opt_trait_ref.as_ref().map(|ast_trait_ref| {
                let selfty = tcx.type_of(def_id);
                AstConv::instantiate_mono_trait_ref(&icx, ast_trait_ref, selfty)
            })
        }
        _ => bug!()
    }
}

fn impl_polarity<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                           def_id: DefId)
                           -> hir::ImplPolarity {
    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();
    match tcx.hir.expect_item(node_id).node {
        hir::ItemImpl(_, polarity, ..) => polarity,
        ref item => bug!("impl_polarity: {:?} not an impl", item)
    }
}

// Is it marked with ?Sized
fn is_unsized<'gcx: 'tcx, 'tcx>(astconv: &AstConv<'gcx, 'tcx>,
                                ast_bounds: &[hir::ParamBound],
                                span: Span) -> bool
{
    let tcx = astconv.tcx();

    // Try to find an unbound in bounds.
    let mut unbound = None;
    for ab in ast_bounds {
        if let &hir::TraitTyParamBound(ref ptr, hir::TraitBoundModifier::Maybe) = ab  {
            if unbound.is_none() {
                unbound = Some(ptr.trait_ref.clone());
            } else {
                span_err!(tcx.sess, span, E0203,
                          "type parameter has more than one relaxed default \
                                                bound, only one is supported");
            }
        }
    }

    let kind_id = tcx.lang_items().require(SizedTraitLangItem);
    match unbound {
        Some(ref tpb) => {
            // FIXME(#8559) currently requires the unbound to be built-in.
            if let Ok(kind_id) = kind_id {
                if tpb.path.def != Def::Trait(kind_id) {
                    tcx.sess.span_warn(span,
                                       "default bound relaxed for a type parameter, but \
                                       this does nothing because the given bound is not \
                                       a default. Only `?Sized` is supported");
                }
            }
        }
        _ if kind_id.is_ok() => {
            return false;
        }
        // No lang item for Sized, so we can't add it as a bound.
        None => {}
    }

    true
}

/// Returns the early-bound lifetimes declared in this generics
/// listing.  For anything other than fns/methods, this is just all
/// the lifetimes that are declared. For fns or methods, we have to
/// screen out those that do not appear in any where-clauses etc using
/// `resolve_lifetime::early_bound_lifetimes`.
fn early_bound_lifetimes_from_generics<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    generics: &'a hir::Generics)
    -> impl Iterator<Item=&'a hir::GenericParam> + Captures<'tcx>
{
    generics.params.iter().filter(move |param| match param.kind {
        GenericParamKind::Lifetime { .. } => {
            let hir_id = tcx.hir.node_to_hir_id(param.id);
            !tcx.is_late_bound(hir_id)
        }
        _ => false,
    })
}

fn predicates_of<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                           def_id: DefId)
                           -> ty::GenericPredicates<'tcx> {
    let explicit = explicit_predicates_of(tcx, def_id);
    let predicates = if tcx.sess.features_untracked().infer_outlives_requirements {
        [&explicit.predicates[..], &tcx.inferred_outlives_of(def_id)[..]].concat()
    } else { explicit.predicates };

    ty::GenericPredicates {
        parent: explicit.parent,
        predicates: predicates,
    }
}

pub fn explicit_predicates_of<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                           def_id: DefId)
                           -> ty::GenericPredicates<'tcx> {
    use rustc::hir::map::*;
    use rustc::hir::*;

    debug!("explicit_predicates_of(def_id={:?})", def_id);

    let node_id = tcx.hir.as_local_node_id(def_id).unwrap();
    let node = tcx.hir.get(node_id);

    let mut is_trait = None;
    let mut is_default_impl_trait = None;

    let icx = ItemCtxt::new(tcx, def_id);
    let no_generics = hir::Generics::empty();
    let ast_generics = match node {
        NodeTraitItem(item) => &item.generics,
        NodeImplItem(item) => &item.generics,

        NodeItem(item) => {
            match item.node {
                ItemImpl(_, _, defaultness, ref generics, ..) => {
                    if defaultness.is_default() {
                        is_default_impl_trait = tcx.impl_trait_ref(def_id);
                    }
                    generics
                }
                ItemFn(.., ref generics, _) |
                ItemTy(_, ref generics) |
                ItemEnum(_, ref generics) |
                ItemStruct(_, ref generics) |
                ItemUnion(_, ref generics) => generics,

                ItemTrait(_, _, ref generics, .., ref items) => {
                    is_trait = Some((ty::TraitRef {
                        def_id,
                        substs: Substs::identity_for_item(tcx, def_id)
                    }, items));
                    generics
                }
                ItemExistential(ref exist_ty) => {
                    let substs = Substs::identity_for_item(tcx, def_id);
                    let anon_ty = tcx.mk_anon(def_id, substs);

                    // Collect the bounds, i.e. the `A+B+'c` in `impl A+B+'c`.
                    let bounds = compute_bounds(&icx,
                                                anon_ty,
                                                &exist_ty.bounds,
                                                SizedByDefault::Yes,
                                                tcx.def_span(def_id));

                    let predicates = bounds.predicates(tcx, anon_ty);

                    return ty::GenericPredicates {
                        parent: None,
                        predicates: predicates
                    };
                }

                _ => &no_generics,
            }
        }

        NodeForeignItem(item) => {
            match item.node {
                ForeignItemStatic(..) => &no_generics,
                ForeignItemFn(_, _, ref generics) => generics,
                ForeignItemType => &no_generics,
            }
        }

        _ => &no_generics,
    };

    let generics = tcx.generics_of(def_id);
    let parent_count = generics.parent_count as u32;
    let has_own_self = generics.has_self && parent_count == 0;

    let mut predicates = vec![];

    // Below we'll consider the bounds on the type parameters (including `Self`)
    // and the explicit where-clauses, but to get the full set of predicates
    // on a trait we need to add in the supertrait bounds and bounds found on
    // associated types.
    if let Some((trait_ref, _)) = is_trait {
        predicates = tcx.super_predicates_of(def_id).predicates;

        // Add in a predicate that `Self:Trait` (where `Trait` is the
        // current trait).  This is needed for builtin bounds.
        predicates.push(trait_ref.to_poly_trait_ref().to_predicate());
    }

    // In default impls, we can assume that the self type implements
    // the trait. So in:
    //
    //     default impl Foo for Bar { .. }
    //
    // we add a default where clause `Foo: Bar`. We do a similar thing for traits
    // (see below). Recall that a default impl is not itself an impl, but rather a
    // set of defaults that can be incorporated into another impl.
    if let Some(trait_ref) = is_default_impl_trait {
        predicates.push(trait_ref.to_poly_trait_ref().to_predicate());
    }

    // Collect the region predicates that were declared inline as
    // well. In the case of parameters declared on a fn or method, we
    // have to be careful to only iterate over early-bound regions.
    let mut index = parent_count + has_own_self as u32;
    for param in early_bound_lifetimes_from_generics(tcx, ast_generics) {
        let region = tcx.mk_region(ty::ReEarlyBound(ty::EarlyBoundRegion {
            def_id: tcx.hir.local_def_id(param.id),
            index,
            name: param.name.name().as_interned_str(),
        }));
        index += 1;

        match param.kind {
            GenericParamKind::Lifetime { .. } => {
                param.bounds.iter().for_each(|bound| match bound {
                    hir::ParamBound::Outlives(lt) => {
                        let bound = AstConv::ast_region_to_region(&icx, &lt, None);
                        let outlives = ty::Binder::bind(ty::OutlivesPredicate(region, bound));
                        predicates.push(outlives.to_predicate());
                    }
                    _ => bug!(),
                });
            },
            _ => bug!(),
        }
    }

    // Collect the predicates that were written inline by the user on each
    // type parameter (e.g., `<T:Foo>`).
    for param in &ast_generics.params {
        match param.kind {
            GenericParamKind::Type { .. } => {
                let name = param.name.name().as_interned_str();
                let param_ty = ty::ParamTy::new(index, name).to_ty(tcx);
                index += 1;

                let sized = SizedByDefault::Yes;
                let bounds = compute_bounds(&icx, param_ty, &param.bounds, sized, param.span);
                predicates.extend(bounds.predicates(tcx, param_ty));
            }
            _ => {}
        }
    }

    // Add in the bounds that appear in the where-clause
    let where_clause = &ast_generics.where_clause;
    for predicate in &where_clause.predicates {
        match predicate {
            &hir::WherePredicate::BoundPredicate(ref bound_pred) => {
                let ty = icx.to_ty(&bound_pred.bounded_ty);

                for bound in bound_pred.bounds.iter() {
                    match bound {
                        &hir::ParamBound::TraitTyParamBound(ref poly_trait_ref, _) => {
                            let mut projections = Vec::new();

                            let trait_ref =
                                AstConv::instantiate_poly_trait_ref(&icx,
                                                                    poly_trait_ref,
                                                                    ty,
                                                                    &mut projections);

                            predicates.push(trait_ref.to_predicate());

                            for projection in &projections {
                                predicates.push(projection.to_predicate());
                            }
                        }

                        &hir::ParamBound::Outlives(ref lifetime) => {
                            let region = AstConv::ast_region_to_region(&icx,
                                                                       lifetime,
                                                                       None);
                            let pred = ty::Binder::bind(ty::OutlivesPredicate(ty, region));
                            predicates.push(ty::Predicate::TypeOutlives(pred))
                        }
                    }
                }
            }

            &hir::WherePredicate::RegionPredicate(ref region_pred) => {
                let r1 = AstConv::ast_region_to_region(&icx, &region_pred.lifetime, None);
                for bound in &region_pred.bounds {
                    let r2 = match bound {
                        hir::ParamBound::Outlives(lt) => {
                            AstConv::ast_region_to_region(&icx, lt, None)
                        }
                        _ => bug!(),
                    };
                    let pred = ty::Binder::bind(ty::OutlivesPredicate(r1, r2));
                    predicates.push(ty::Predicate::RegionOutlives(pred))
                }
            }

            &hir::WherePredicate::EqPredicate(..) => {
                // FIXME(#20041)
            }
        }
    }

    // Add predicates from associated type bounds.
    if let Some((self_trait_ref, trait_items)) = is_trait {
        predicates.extend(trait_items.iter().flat_map(|trait_item_ref| {
            let trait_item = tcx.hir.trait_item(trait_item_ref.id);
            let bounds = match trait_item.node {
                hir::TraitItemKind::Type(ref bounds, _) => bounds,
                _ => {
                    return vec![].into_iter();
                }
            };

            let assoc_ty = tcx.mk_projection(
                tcx.hir.local_def_id(trait_item.id),
                self_trait_ref.substs,
            );

            let bounds = compute_bounds(&ItemCtxt::new(tcx, def_id),
                                        assoc_ty,
                                        bounds,
                                        SizedByDefault::Yes,
                                        trait_item.span);

            bounds.predicates(tcx, assoc_ty).into_iter()
        }))
    }

    // Subtle: before we store the predicates into the tcx, we
    // sort them so that predicates like `T: Foo<Item=U>` come
    // before uses of `U`.  This avoids false ambiguity errors
    // in trait checking. See `setup_constraining_predicates`
    // for details.
    if let NodeItem(&Item { node: ItemImpl(..), .. }) = node {
        let self_ty = tcx.type_of(def_id);
        let trait_ref = tcx.impl_trait_ref(def_id);
        ctp::setup_constraining_predicates(tcx,
                                           &mut predicates,
                                           trait_ref,
                                           &mut ctp::parameters_for_impl(self_ty, trait_ref));
    }

    ty::GenericPredicates {
        parent: generics.parent,
        predicates,
    }
}

pub enum SizedByDefault { Yes, No, }

/// Translate the AST's notion of ty param bounds (which are an enum consisting of a newtyped Ty or
/// a region) to ty's notion of ty param bounds, which can either be user-defined traits, or the
/// built-in trait (formerly known as kind): Send.
pub fn compute_bounds<'gcx: 'tcx, 'tcx>(astconv: &AstConv<'gcx, 'tcx>,
                                        param_ty: Ty<'tcx>,
                                        ast_bounds: &[hir::ParamBound],
                                        sized_by_default: SizedByDefault,
                                        span: Span)
                                        -> Bounds<'tcx>
{
    let mut region_bounds = vec![];
    let mut trait_bounds = vec![];
    for ast_bound in ast_bounds {
        match *ast_bound {
            hir::TraitTyParamBound(ref b, hir::TraitBoundModifier::None) => {
                trait_bounds.push(b);
            }
            hir::TraitTyParamBound(_, hir::TraitBoundModifier::Maybe) => {}
            hir::Outlives(ref l) => {
                region_bounds.push(l);
            }
        }
    }

    let mut projection_bounds = vec![];

    let mut trait_bounds: Vec<_> = trait_bounds.iter().map(|&bound| {
        astconv.instantiate_poly_trait_ref(bound,
                                           param_ty,
                                           &mut projection_bounds)
    }).collect();

    let region_bounds = region_bounds.into_iter().map(|r| {
        astconv.ast_region_to_region(r, None)
    }).collect();

    trait_bounds.sort_by(|a,b| a.def_id().cmp(&b.def_id()));

    let implicitly_sized = if let SizedByDefault::Yes = sized_by_default {
        !is_unsized(astconv, ast_bounds, span)
    } else {
        false
    };

    Bounds {
        region_bounds,
        implicitly_sized,
        trait_bounds,
        projection_bounds,
    }
}

/// Converts a specific ParamBound from the AST into a set of
/// predicates that apply to the self-type. A vector is returned
/// because this can be anywhere from 0 predicates (`T:?Sized` adds no
/// predicates) to 1 (`T:Foo`) to many (`T:Bar<X=i32>` adds `T:Bar`
/// and `<T as Bar>::X == i32`).
fn predicates_from_bound<'tcx>(astconv: &AstConv<'tcx, 'tcx>,
                               param_ty: Ty<'tcx>,
                               bound: &hir::ParamBound)
                               -> Vec<ty::Predicate<'tcx>>
{
    match *bound {
        hir::TraitTyParamBound(ref tr, hir::TraitBoundModifier::None) => {
            let mut projections = Vec::new();
            let pred = astconv.instantiate_poly_trait_ref(tr,
                                                          param_ty,
                                                          &mut projections);
            projections.into_iter()
                       .map(|p| p.to_predicate())
                       .chain(Some(pred.to_predicate()))
                       .collect()
        }
        hir::Outlives(ref lifetime) => {
            let region = astconv.ast_region_to_region(lifetime, None);
            let pred = ty::Binder::bind(ty::OutlivesPredicate(param_ty, region));
            vec![ty::Predicate::TypeOutlives(pred)]
        }
        hir::TraitTyParamBound(_, hir::TraitBoundModifier::Maybe) => {
            Vec::new()
        }
    }
}

fn compute_sig_of_foreign_fn_decl<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    def_id: DefId,
    decl: &hir::FnDecl,
    abi: abi::Abi)
    -> ty::PolyFnSig<'tcx>
{
    let fty = AstConv::ty_of_fn(&ItemCtxt::new(tcx, def_id), hir::Unsafety::Unsafe, abi, decl);

    // feature gate SIMD types in FFI, since I (huonw) am not sure the
    // ABIs are handled at all correctly.
    if abi != abi::Abi::RustIntrinsic && abi != abi::Abi::PlatformIntrinsic
            && !tcx.features().simd_ffi {
        let check = |ast_ty: &hir::Ty, ty: Ty| {
            if ty.is_simd() {
                tcx.sess.struct_span_err(ast_ty.span,
                              &format!("use of SIMD type `{}` in FFI is highly experimental and \
                                        may result in invalid code",
                                       tcx.hir.node_to_pretty_string(ast_ty.id)))
                    .help("add #![feature(simd_ffi)] to the crate attributes to enable")
                    .emit();
            }
        };
        for (input, ty) in decl.inputs.iter().zip(*fty.inputs().skip_binder()) {
            check(&input, ty)
        }
        if let hir::Return(ref ty) = decl.output {
            check(&ty, *fty.output().skip_binder())
        }
    }

    fty
}

fn is_foreign_item<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                             def_id: DefId)
                             -> bool {
    match tcx.hir.get_if_local(def_id) {
        Some(hir_map::NodeForeignItem(..)) => true,
        Some(_) => false,
        _ => bug!("is_foreign_item applied to non-local def-id {:?}", def_id)
    }
}

fn from_target_feature(
    tcx: TyCtxt,
    id: DefId,
    attr: &ast::Attribute,
    whitelist: &FxHashMap<String, Option<String>>,
    target_features: &mut Vec<Symbol>,
) {
    let list = match attr.meta_item_list() {
        Some(list) => list,
        None => {
            let msg = "#[target_feature] attribute must be of the form \
                       #[target_feature(..)]";
            tcx.sess.span_err(attr.span, &msg);
            return
        }
    };
    let rust_features = tcx.features();
    for item in list {
        // Only `enable = ...` is accepted in the meta item list
        if !item.check_name("enable") {
            let msg = "#[target_feature(..)] only accepts sub-keys of `enable` \
                       currently";
            tcx.sess.span_err(item.span, &msg);
            continue
        }

        // Must be of the form `enable = "..."` ( a string)
        let value = match item.value_str() {
            Some(value) => value,
            None => {
                let msg = "#[target_feature] attribute must be of the form \
                           #[target_feature(enable = \"..\")]";
                tcx.sess.span_err(item.span, &msg);
                continue
            }
        };

        // We allow comma separation to enable multiple features
        for feature in value.as_str().split(',') {

            // Only allow whitelisted features per platform
            let feature_gate = match whitelist.get(feature) {
                Some(g) => g,
                None => {
                    let msg = format!("the feature named `{}` is not valid for \
                                       this target", feature);
                    let mut err = tcx.sess.struct_span_err(item.span, &msg);

                    if feature.starts_with("+") {
                        let valid = whitelist.contains_key(&feature[1..]);
                        if valid {
                            err.help("consider removing the leading `+` in the feature name");
                        }
                    }
                    err.emit();
                    continue
                }
            };

            // Only allow features whose feature gates have been enabled
            let allowed = match feature_gate.as_ref().map(|s| &**s) {
                Some("arm_target_feature") => rust_features.arm_target_feature,
                Some("aarch64_target_feature") => rust_features.aarch64_target_feature,
                Some("hexagon_target_feature") => rust_features.hexagon_target_feature,
                Some("powerpc_target_feature") => rust_features.powerpc_target_feature,
                Some("mips_target_feature") => rust_features.mips_target_feature,
                Some("avx512_target_feature") => rust_features.avx512_target_feature,
                Some("mmx_target_feature") => rust_features.mmx_target_feature,
                Some("sse4a_target_feature") => rust_features.sse4a_target_feature,
                Some("tbm_target_feature") => rust_features.tbm_target_feature,
                Some(name) => bug!("unknown target feature gate {}", name),
                None => true,
            };
            if !allowed && id.is_local() {
                feature_gate::emit_feature_err(
                    &tcx.sess.parse_sess,
                    feature_gate.as_ref().unwrap(),
                    item.span,
                    feature_gate::GateIssue::Language,
                    &format!("the target feature `{}` is currently unstable",
                             feature),
                );
                continue
            }
            target_features.push(Symbol::intern(feature));
        }
    }
}

fn linkage_by_name<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, def_id: DefId, name: &str) -> Linkage {
    use rustc::mir::mono::Linkage::*;

    // Use the names from src/llvm/docs/LangRef.rst here. Most types are only
    // applicable to variable declarations and may not really make sense for
    // Rust code in the first place but whitelist them anyway and trust that
    // the user knows what s/he's doing. Who knows, unanticipated use cases
    // may pop up in the future.
    //
    // ghost, dllimport, dllexport and linkonce_odr_autohide are not supported
    // and don't have to be, LLVM treats them as no-ops.
    match name {
        "appending" => Appending,
        "available_externally" => AvailableExternally,
        "common" => Common,
        "extern_weak" => ExternalWeak,
        "external" => External,
        "internal" => Internal,
        "linkonce" => LinkOnceAny,
        "linkonce_odr" => LinkOnceODR,
        "private" => Private,
        "weak" => WeakAny,
        "weak_odr" => WeakODR,
        _ => {
            let span = tcx.hir.span_if_local(def_id);
            if let Some(span) = span {
                tcx.sess.span_fatal(span, "invalid linkage specified")
            } else {
                tcx.sess.fatal(&format!("invalid linkage specified: {}", name))
            }
        }
    }
}

fn codegen_fn_attrs<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, id: DefId) -> CodegenFnAttrs {
    let attrs = tcx.get_attrs(id);

    let mut codegen_fn_attrs = CodegenFnAttrs::new();

    let whitelist = tcx.target_features_whitelist(LOCAL_CRATE);

    let mut inline_span = None;
    for attr in attrs.iter() {
        if attr.check_name("cold") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::COLD;
        } else if attr.check_name("allocator") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::ALLOCATOR;
        } else if attr.check_name("unwind") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::UNWIND;
        } else if attr.check_name("rustc_allocator_nounwind") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::RUSTC_ALLOCATOR_NOUNWIND;
        } else if attr.check_name("naked") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::NAKED;
        } else if attr.check_name("no_mangle") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_MANGLE;
        } else if attr.check_name("rustc_std_internal_symbol") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::RUSTC_STD_INTERNAL_SYMBOL;
        } else if attr.check_name("no_debug") {
            codegen_fn_attrs.flags |= CodegenFnAttrFlags::NO_DEBUG;
        } else if attr.check_name("inline") {
            codegen_fn_attrs.inline = attrs.iter().fold(InlineAttr::None, |ia, attr| {
                if attr.path != "inline" {
                    return ia;
                }
                let meta = match attr.meta() {
                    Some(meta) => meta.node,
                    None => return ia,
                };
                match meta {
                    MetaItemKind::Word => {
                        mark_used(attr);
                        InlineAttr::Hint
                    }
                    MetaItemKind::List(ref items) => {
                        mark_used(attr);
                        inline_span = Some(attr.span);
                        if items.len() != 1 {
                            span_err!(tcx.sess.diagnostic(), attr.span, E0534,
                                        "expected one argument");
                            InlineAttr::None
                        } else if list_contains_name(&items[..], "always") {
                            InlineAttr::Always
                        } else if list_contains_name(&items[..], "never") {
                            InlineAttr::Never
                        } else {
                            span_err!(tcx.sess.diagnostic(), items[0].span, E0535,
                                        "invalid argument");

                            InlineAttr::None
                        }
                    }
                    _ => ia,
                }
            });
        } else if attr.check_name("export_name") {
            if let s @ Some(_) = attr.value_str() {
                codegen_fn_attrs.export_name = s;
            } else {
                struct_span_err!(tcx.sess, attr.span, E0558,
                                    "export_name attribute has invalid format")
                    .span_label(attr.span, "did you mean #[export_name=\"*\"]?")
                    .emit();
            }
        } else if attr.check_name("target_feature") {
            if tcx.fn_sig(id).unsafety() == Unsafety::Normal {
                let msg = "#[target_feature(..)] can only be applied to \
                           `unsafe` function";
                tcx.sess.span_err(attr.span, msg);
            }
            from_target_feature(tcx, id, attr, &whitelist, &mut codegen_fn_attrs.target_features);
        } else if attr.check_name("linkage") {
            if let Some(val) = attr.value_str() {
                codegen_fn_attrs.linkage = Some(linkage_by_name(tcx, id, &val.as_str()));
            }
        }
    }

    // If a function uses #[target_feature] it can't be inlined into general
    // purpose functions as they wouldn't have the right target features
    // enabled. For that reason we also forbid #[inline(always)] as it can't be
    // respected.
    if codegen_fn_attrs.target_features.len() > 0 {
        if codegen_fn_attrs.inline == InlineAttr::Always {
            if let Some(span) = inline_span {
                tcx.sess.span_err(span, "cannot use #[inline(always)] with \
                                         #[target_feature]");
            }
        }
    }

    codegen_fn_attrs
}
