// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Error Reporting for Anonymous Region Lifetime Errors
//! where both the regions are anonymous.
use hir;
use infer::InferCtxt;
use ty::{self, Region};
use infer::region_inference::RegionResolutionError::*;
use infer::region_inference::RegionResolutionError;
use hir::map as hir_map;
use middle::resolve_lifetime as rl;
use hir::intravisit::{self, Visitor, NestedVisitorMap};

impl<'a, 'gcx, 'tcx> InferCtxt<'a, 'gcx, 'tcx> {
    // This method prints the error message for lifetime errors when both the concerned regions
    // are anonymous.
    // Consider a case where we have
    // fn foo(x: &mut Vec<&u8>, y: &u8)
    //    { x.push(y); }.
    // The example gives
    // fn foo(x: &mut Vec<&u8>, y: &u8) {
    //                    ---      --- these references are declared with different lifetimes...
    //            x.push(y);
    //            ^ ...but data from `y` flows into `x` here
    // It has been extended for the case of structs too.
    // Consider the example
    // struct Ref<'a> { x: &'a u32 }
    // fn foo(mut x: Vec<Ref>, y: Ref) {
    //                   ---      --- these structs are declared with different lifetimes...
    //               x.push(y);
    //               ^ ...but data from `y` flows into `x` here
    // }
    // It will later be extended to trait objects.
    pub fn try_report_anon_anon_conflict(&self, error: &RegionResolutionError<'tcx>) -> bool {
        let (span, sub, sup) = match *error {
            ConcreteFailure(ref origin, sub, sup) => (origin.span(), sub, sup),
            _ => return false, // inapplicable
        };

        // Determine whether the sub and sup consist of both anonymous (elided) regions.
        let (ty1, ty2, scope_def_id_1, scope_def_id_2, bregion1, bregion2) = if
            self.is_suitable_anonymous_region(sup, true).is_some() &&
            self.is_suitable_anonymous_region(sub, true).is_some() {
            if let (Some(anon_reg1), Some(anon_reg2)) =
                (self.is_suitable_anonymous_region(sup, true),
                 self.is_suitable_anonymous_region(sub, true)) {
                let ((def_id1, br1), (def_id2, br2)) = (anon_reg1, anon_reg2);
                let found_arg1 = self.find_anon_type(sup, &br1);
                let found_arg2 = self.find_anon_type(sub, &br2);
                match (found_arg1, found_arg2) {
                    (Some(anonarg_1), Some(anonarg_2)) => {
                        (anonarg_1, anonarg_2, def_id1, def_id2, br1, br2)
                    }
                    _ => {
                        return false;
                    }
                }

            } else {
                return false;
            }
        } else {
            return false; //inapplicable
        };

        let (label1, label2) = if let (Some(sup_arg), Some(sub_arg)) =
            (self.find_arg_with_anonymous_region(sup, sup),
             self.find_arg_with_anonymous_region(sub, sub)) {

            let ((anon_arg1, _, _, is_first1), (anon_arg2, _, _, is_first2)) = (sup_arg, sub_arg);
            if self.is_self_anon(is_first1, scope_def_id_1) ||
               self.is_self_anon(is_first2, scope_def_id_2) {
                return false;
            }

            if self.is_return_type_anon(scope_def_id_1, bregion1) ||
               self.is_return_type_anon(scope_def_id_2, bregion2) {
                return false;
            }




            if anon_arg1 == anon_arg2 {
                (format!(" with one lifetime"), format!(" into the other"))
            } else {
                let span_label_var1 = if let Some(simple_name) = anon_arg1.pat.simple_name() {
                    format!(" from `{}`", simple_name)
                } else {
                    format!("")
                };

                let span_label_var2 = if let Some(simple_name) = anon_arg2.pat.simple_name() {
                    format!(" into `{}`", simple_name)
                } else {
                    format!("")
                };

                (span_label_var1, span_label_var2)
            }
        } else {
            return false;
        };

        struct_span_err!(self.tcx.sess, span, E0623, "lifetime mismatch")
            .span_label(ty1.span,
                        format!("these two types are declared with different lifetimes..."))
            .span_label(ty2.span, format!(""))
            .span_label(span, format!("...but data{} flows{} here", label1, label2))
            .emit();
        return true;

    }

    /// This function calls the `visit_ty` method for the parameters
    /// corresponding to the anonymous regions. The `nested_visitor.found_type`
    /// contains the anonymous type.
    ///
    /// # Arguments
    ///
    /// region - the anonymous region corresponding to the anon_anon conflict
    /// br - the bound region corresponding to the above region which is of type `BrAnon(_)`
    ///
    /// # Example
    /// ```
    /// fn foo(x: &mut Vec<&u8>, y: &u8)
    ///    { x.push(y); }
    /// ```
    /// The function returns the nested type corresponding to the anonymous region
    /// for e.g. `&u8` and Vec<`&u8`.
    pub fn find_anon_type(&self, region: Region<'tcx>, br: &ty::BoundRegion) -> Option<(&hir::Ty)> {
        if let Some(anon_reg) = self.is_suitable_anonymous_region(region, true) {
            let (def_id, _) = anon_reg;
            if let Some(node_id) = self.tcx.hir.as_local_node_id(def_id) {
                let ret_ty = self.tcx.type_of(def_id);
                if let ty::TyFnDef(_, _) = ret_ty.sty {
                    if let hir_map::NodeItem(it) = self.tcx.hir.get(node_id) {
                        if let hir::ItemFn(ref fndecl, _, _, _, _, _) = it.node {
                            return fndecl
                                       .inputs
                                       .iter()
                                       .filter_map(|arg| {
                                                       self.find_visitor_found_type(&**arg, br)
                                                   })
                                       .next();
                        }
                    } else if let hir_map::NodeTraitItem(it) = self.tcx.hir.get(node_id) {
                        if let hir::TraitItemKind::Method(ref fndecl, _) = it.node {
                            return fndecl
                                       .decl
                                       .inputs
                                       .iter()
                                       .filter_map(|arg| {
                                                       self.find_visitor_found_type(&**arg, br)
                                                   })
                                       .next();
                        }
                    } else if let hir_map::NodeImplItem(it) = self.tcx.hir.get(node_id) {
                        if let hir::ImplItemKind::Method(ref fndecl, _) = it.node {
                            return fndecl
                                       .decl
                                       .inputs
                                       .iter()
                                       .filter_map(|arg| {
                                                       self.find_visitor_found_type(&**arg, br)
                                                   })
                                       .next();
                        }
                    }
                }
            }
        }
        None
    }

    // This method creates a FindNestedTypeVisitor which returns the type corresponding
    // to the anonymous region.
    fn find_visitor_found_type(&self,
                               arg: &'gcx hir::Ty,
                               br: &ty::BoundRegion)
                               -> Option<(&'gcx hir::Ty)> {
        let mut nested_visitor = FindNestedTypeVisitor {
            infcx: &self,
            hir_map: &self.tcx.hir,
            bound_region: *br,
            found_type: None,
            depth: 0,
        };
        nested_visitor.visit_ty(arg);
        nested_visitor.found_type
    }
}

// The FindNestedTypeVisitor captures the corresponding `hir::Ty` of the
// anonymous region. The example above would lead to a conflict between
// the two anonymous lifetimes for &u8 in x and y respectively. This visitor
// would be invoked twice, once for each lifetime, and would
// walk the types like &mut Vec<&u8> and &u8 looking for the HIR
// where that lifetime appears. This allows us to highlight the
// specific part of the type in the error message.
struct FindNestedTypeVisitor<'a, 'gcx: 'a + 'tcx, 'tcx: 'a> {
    infcx: &'a InferCtxt<'a, 'gcx, 'tcx>,
    hir_map: &'a hir::map::Map<'gcx>,
    // The bound_region corresponding to the Refree(freeregion)
    // associated with the anonymous region we are looking for.
    bound_region: ty::BoundRegion,
    // The type where the anonymous lifetime appears
    // for e.g. Vec<`&u8`> and <`&u8`>
    found_type: Option<&'gcx hir::Ty>,
    depth: u32,
}

impl<'a, 'gcx, 'tcx> Visitor<'gcx> for FindNestedTypeVisitor<'a, 'gcx, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'gcx> {
        NestedVisitorMap::OnlyBodies(&self.hir_map)
    }

    fn visit_ty(&mut self, arg: &'gcx hir::Ty) {
        // Find the index of the anonymous region that was part of the
        // error. We will then search the function parameters for a bound
        // region at the right depth with the same index.
        let br_index = match self.bound_region {
            ty::BrAnon(index) => index,
            _ => return,
        };

        match arg.node {
            hir::TyRptr(ref lifetime, _) => {
                match self.infcx.tcx.named_region_map.defs.get(&lifetime.id) {
                    // the lifetime of the TyRptr
                    Some(&rl::Region::LateBoundAnon(debuijn_index, anon_index)) => {
                        if debuijn_index.depth == 1 && anon_index == br_index {
                            self.found_type = Some(arg);
                            return; // we can stop visiting now
                        }
                    }
                    Some(&rl::Region::Static) |
                    Some(&rl::Region::EarlyBound(_, _)) |
                    Some(&rl::Region::LateBound(_, _)) |
                    Some(&rl::Region::Free(_, _)) |
                    None => {
                        debug!("no arg found");
                    }
                }
            }
            // Checks if it is of type `hir::TyPath` which corresponds to a struct.
            hir::TyPath(_) => {
                let subvisitor = &mut TyPathVisitor {
                                          infcx: self.infcx,
                                          found_it: false,
                                          bound_region: self.bound_region,
                                          hir_map: self.hir_map,
                                      };
                intravisit::walk_ty(subvisitor, arg); // call walk_ty; as visit_ty is empty,
                // this will visit only outermost type
                if subvisitor.found_it {
                    self.found_type = Some(arg);
                }
            }

            hir::TyBareFn(ref fndecl) => {
                fndecl.lifetimes.iter().map(|lf| {
                                            debug!("arg we are handling is...{:?}",arg);
                    match self.infcx.tcx.named_region_map.defs.get(&lf.lifetime.id) {
                        Some(&rl::Region::LateBoundAnon(debuijn_index, anon_index)) => {
                            debug!("debuijn_index.depth ={:?} self.depth = {:?} anon_index ={:?} br_index={:?}",
                            debuijn_index.depth, self.depth, anon_index, br_index);
                        if debuijn_index.depth == self.depth && anon_index == br_index {
                            debug!("arg is {:?}",Some(arg));
                            self.found_type = Some(arg);
                            return; // we can stop visiting now
                        }
                    }
                    Some(&rl::Region::Static) |
                    Some(&rl::Region::EarlyBound(_, _)) |
                    Some(&rl::Region::LateBound(_, _)) |
                    Some(&rl::Region::Free(_, _)) |
                    None => {
                        debug!("no arg found");
                    }
                }       
            
            }).next();}
            
            _ => {}
        }
        // walk the embedded contents: e.g., if we are visiting `Vec<&Foo>`,
        // go on to visit `&Foo`
        self.depth += 1;
        debug!("depth is {:?}",self.depth);
        intravisit::walk_ty(self, arg);
        self.depth += 1;
    }
}

// The visitor captures the corresponding `hir::Ty` of the anonymous region
// in the case of structs ie. `hir::TyPath`.
// This visitor would be invoked for each lifetime corresponding to a struct,
// and would walk the types like Vec<Ref> in the above example and Ref looking for the HIR
// where that lifetime appears. This allows us to highlight the
// specific part of the type in the error message.
struct TyPathVisitor<'a, 'gcx: 'a + 'tcx, 'tcx: 'a> {
    infcx: &'a InferCtxt<'a, 'gcx, 'tcx>,
    hir_map: &'a hir::map::Map<'gcx>,
    found_it: bool,
    bound_region: ty::BoundRegion,
}

impl<'a, 'gcx, 'tcx> Visitor<'gcx> for TyPathVisitor<'a, 'gcx, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'gcx> {
        NestedVisitorMap::OnlyBodies(&self.hir_map)
    }

    fn visit_lifetime(&mut self, lifetime: &hir::Lifetime) {
        let br_index = match self.bound_region {
            ty::BrAnon(index) => index,
            _ => return,
        };


        match self.infcx.tcx.named_region_map.defs.get(&lifetime.id) {
            // the lifetime of the TyPath!
            Some(&rl::Region::LateBoundAnon(debuijn_index, anon_index)) => {
                if debuijn_index.depth == 1 && anon_index == br_index {
                    self.found_it = true;
                }
            }
            Some(&rl::Region::Static) |
            Some(&rl::Region::EarlyBound(_, _)) |
            Some(&rl::Region::LateBound(_, _)) |
            Some(&rl::Region::Free(_, _)) |
            None => {
                debug!("no arg found");
            }
        }
    }

    fn visit_ty(&mut self, arg: &'gcx hir::Ty) {
        // ignore nested types
        //
        // If you have a type like `Foo<'a, &Ty>` we
        // are only interested in the immediate lifetimes ('a).
        //
        // Making `visit_ty` empty will ignore the `&Ty` embedded
        // inside, it will get reached by the outer visitor.
        debug!("`Ty` corresponding to a struct is {:?}", arg);
    }
}
