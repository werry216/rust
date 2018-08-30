// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clean::*;

use super::Pass;
use core::DocContext;
use fold::DocFolder;

pub const COLLECT_TRAIT_IMPLS: Pass =
    Pass::early("collect-trait-impls", collect_trait_impls,
                "retrieves trait impls for items in the crate");

pub fn collect_trait_impls(krate: Crate, cx: &DocContext) -> Crate {
    let mut synth = SyntheticImplCollector::new(cx);
    let mut krate = synth.fold_crate(krate);

    if let Some(ref mut it) = krate.module {
        if let ModuleItem(Module { ref mut items, .. }) = it.inner {
            items.extend(synth.impls);

            for &cnum in cx.tcx.crates().iter() {
                for &did in cx.tcx.all_trait_implementations(cnum).iter() {
                    inline::build_impl(cx, did, items);
                }
            }

            // `tcx.crates()` doesn't include the local crate, and `tcx.all_trait_implementations`
            // doesn't work with it anyway, so pull them from the HIR map instead
            for &trait_did in cx.all_traits.iter() {
                for &impl_node in cx.tcx.hir.trait_impls(trait_did) {
                    let impl_did = cx.tcx.hir.local_def_id(impl_node);
                    inline::build_impl(cx, impl_did, items);
                }
            }

            // Also try to inline primitive impls from other crates.
            let lang_items = cx.tcx.lang_items();
            let primitive_impls = [
                lang_items.isize_impl(),
                lang_items.i8_impl(),
                lang_items.i16_impl(),
                lang_items.i32_impl(),
                lang_items.i64_impl(),
                lang_items.i128_impl(),
                lang_items.usize_impl(),
                lang_items.u8_impl(),
                lang_items.u16_impl(),
                lang_items.u32_impl(),
                lang_items.u64_impl(),
                lang_items.u128_impl(),
                lang_items.f32_impl(),
                lang_items.f64_impl(),
                lang_items.f32_runtime_impl(),
                lang_items.f64_runtime_impl(),
                lang_items.char_impl(),
                lang_items.str_impl(),
                lang_items.slice_impl(),
                lang_items.slice_u8_impl(),
                lang_items.str_alloc_impl(),
                lang_items.slice_alloc_impl(),
                lang_items.slice_u8_alloc_impl(),
                lang_items.const_ptr_impl(),
                lang_items.mut_ptr_impl(),
            ];

            for def_id in primitive_impls.iter().filter_map(|&def_id| def_id) {
                if !def_id.is_local() {
                    inline::build_impl(cx, def_id, items);

                    let auto_impls = get_auto_traits_with_def_id(cx, def_id);
                    let blanket_impls = get_blanket_impls_with_def_id(cx, def_id);
                    let mut renderinfo = cx.renderinfo.borrow_mut();

                    let new_impls: Vec<Item> = auto_impls.into_iter()
                        .chain(blanket_impls.into_iter())
                        .filter(|i| renderinfo.inlined.insert(i.def_id))
                        .collect();

                    items.extend(new_impls);
                }
            }
        } else {
            panic!("collect-trait-impls can't run");
        }
    } else {
        panic!("collect-trait-impls can't run");
    }

    krate
}

struct SyntheticImplCollector<'a, 'tcx: 'a, 'rcx: 'a, 'cstore: 'rcx> {
    cx: &'a DocContext<'a, 'tcx, 'rcx, 'cstore>,
    impls: Vec<Item>,
}

impl<'a, 'tcx, 'rcx, 'cstore> SyntheticImplCollector<'a, 'tcx, 'rcx, 'cstore> {
    fn new(cx: &'a DocContext<'a, 'tcx, 'rcx, 'cstore>) -> Self {
        SyntheticImplCollector {
            cx,
            impls: Vec::new(),
        }
    }
}

impl<'a, 'tcx, 'rcx, 'cstore> DocFolder for SyntheticImplCollector<'a, 'tcx, 'rcx, 'cstore> {
    fn fold_item(&mut self, i: Item) -> Option<Item> {
        if i.is_struct() || i.is_enum() || i.is_union() {
            if let (Some(node_id), Some(name)) =
                (self.cx.tcx.hir.as_local_node_id(i.def_id), i.name.clone())
            {
                self.impls.extend(get_auto_traits_with_node_id(self.cx, node_id, name.clone()));
                self.impls.extend(get_blanket_impls_with_node_id(self.cx, node_id, name));
            } else {
                self.impls.extend(get_auto_traits_with_def_id(self.cx, i.def_id));
                self.impls.extend(get_blanket_impls_with_def_id(self.cx, i.def_id));
            }
        }

        self.fold_item_recur(i)
    }
}
