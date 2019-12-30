use crate::utils::{match_def_path, paths, span_lint, trait_ref_of_method, walk_ptrs_ty};
use rustc::declare_lint_pass;
use rustc::hir;
use rustc::lint::{LateContext, LateLintPass, LintArray, LintPass};
use rustc::ty::{Adt, Dynamic, Opaque, Param, RawPtr, Ref, Ty, TypeAndMut};
use rustc_session::declare_tool_lint;
use syntax::source_map::Span;

declare_clippy_lint! {
    /// **What it does:** Checks for sets/maps with mutable key types.
    ///
    /// **Why is this bad?** All of `HashMap`, `HashSet`, `BTreeMap` and
    /// `BtreeSet` rely on either the hash or the order of keys be unchanging,
    /// so having types with interior mutability is a bad idea.
    ///
    /// **Known problems:** We don't currently account for `Rc` or `Arc`, so
    /// this may yield false positives.
    ///
    /// **Example:**
    /// ```rust
    /// use std::cmp::{PartialEq, Eq};
    /// use std::collections::HashSet;
    /// use std::hash::{Hash, Hasher};
    /// use std::sync::atomic::AtomicUsize;
    ///# #[allow(unused)]
    ///
    /// struct Bad(AtomicUsize);
    /// impl PartialEq for Bad {
    ///     fn eq(&self, rhs: &Self) -> bool {
    ///          ..
    /// ; unimplemented!();
    ///     }
    /// }
    ///
    /// impl Eq for Bad {}
    ///
    /// impl Hash for Bad {
    ///     fn hash<H: Hasher>(&self, h: &mut H) {
    ///         ..
    /// ; unimplemented!();
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let _: HashSet<Bad> = HashSet::new();
    /// }
    /// ```
    pub MUTABLE_KEY_TYPE,
    correctness,
    "Check for mutable Map/Set key type"
}

declare_lint_pass!(MutableKeyType => [ MUTABLE_KEY_TYPE ]);

impl<'a, 'tcx> LateLintPass<'a, 'tcx> for MutableKeyType {
    fn check_item(&mut self, cx: &LateContext<'a, 'tcx>, item: &'tcx hir::Item<'tcx>) {
        if let hir::ItemKind::Fn(ref sig, ..) = item.kind {
            check_sig(cx, item.hir_id, &sig.decl);
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'a, 'tcx>, item: &'tcx hir::ImplItem<'tcx>) {
        if let hir::ImplItemKind::Method(ref sig, ..) = item.kind {
            if trait_ref_of_method(cx, item.hir_id).is_none() {
                check_sig(cx, item.hir_id, &sig.decl);
            }
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'a, 'tcx>, item: &'tcx hir::TraitItem<'tcx>) {
        if let hir::TraitItemKind::Method(ref sig, ..) = item.kind {
            check_sig(cx, item.hir_id, &sig.decl);
        }
    }

    fn check_local(&mut self, cx: &LateContext<'_, '_>, local: &hir::Local<'_>) {
        if let hir::PatKind::Wild = local.pat.kind {
            return;
        }
        check_ty(cx, local.span, cx.tables.pat_ty(&*local.pat));
    }
}

fn check_sig<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, item_hir_id: hir::HirId, decl: &hir::FnDecl<'_>) {
    let fn_def_id = cx.tcx.hir().local_def_id(item_hir_id);
    let fn_sig = cx.tcx.fn_sig(fn_def_id);
    for (hir_ty, ty) in decl.inputs.iter().zip(fn_sig.inputs().skip_binder().iter()) {
        check_ty(cx, hir_ty.span, ty);
    }
    check_ty(
        cx,
        decl.output.span(),
        cx.tcx.erase_late_bound_regions(&fn_sig.output()),
    );
}

// We want to lint 1. sets or maps with 2. not immutable key types and 3. no unerased
// generics (because the compiler cannot ensure immutability for unknown types).
fn check_ty<'a, 'tcx>(cx: &LateContext<'a, 'tcx>, span: Span, ty: Ty<'tcx>) {
    let ty = walk_ptrs_ty(ty);
    if let Adt(def, substs) = ty.kind {
        if [&paths::HASHMAP, &paths::BTREEMAP, &paths::HASHSET, &paths::BTREESET]
            .iter()
            .any(|path| match_def_path(cx, def.did, &**path))
        {
            let key_type = concrete_type(substs.type_at(0));
            if let Some(key_type) = key_type {
                if !key_type.is_freeze(cx.tcx, cx.param_env, span) {
                    span_lint(cx, MUTABLE_KEY_TYPE, span, "mutable key type");
                }
            }
        }
    }
}

fn concrete_type(ty: Ty<'_>) -> Option<Ty<'_>> {
    match ty.kind {
        RawPtr(TypeAndMut { ty: inner_ty, .. }) | Ref(_, inner_ty, _) => concrete_type(inner_ty),
        Dynamic(..) | Opaque(..) | Param(..) => None,
        _ => Some(ty),
    }
}
