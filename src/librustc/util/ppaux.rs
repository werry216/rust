use crate::hir::def_id::DefId;
use crate::hir::map::definitions::DefPathData;
use crate::middle::region;
use crate::ty::subst::{self, Subst, SubstsRef};
use crate::ty::{BrAnon, BrEnv, BrFresh, BrNamed};
use crate::ty::{Bool, Char, Adt};
use crate::ty::{Error, Str, Array, Slice, Float, FnDef, FnPtr};
use crate::ty::{Param, Bound, RawPtr, Ref, Never, Tuple};
use crate::ty::{Closure, Generator, GeneratorWitness, Foreign, Projection, Opaque};
use crate::ty::{Placeholder, UnnormalizedProjection, Dynamic, Int, Uint, Infer};
use crate::ty::{self, Ty, TypeFoldable, GenericParamCount, GenericParamDefKind, ParamConst};
use crate::ty::print::{PrintCx, Print};
use crate::mir::interpret::ConstValue;

use std::cell::Cell;
use std::fmt;
use std::usize;

use rustc_target::spec::abi::Abi;
use syntax::ast::CRATE_NODE_ID;
use syntax::symbol::{Symbol, InternedString};
use crate::hir;

/// The "region highlights" are used to control region printing during
/// specific error messages. When a "region highlight" is enabled, it
/// gives an alternate way to print specific regions. For now, we
/// always print those regions using a number, so something like "`'0`".
///
/// Regions not selected by the region highlight mode are presently
/// unaffected.
#[derive(Copy, Clone, Default)]
pub struct RegionHighlightMode {
    /// If enabled, when we see the selected region, use "`'N`"
    /// instead of the ordinary behavior.
    highlight_regions: [Option<(ty::RegionKind, usize)>; 3],

    /// If enabled, when printing a "free region" that originated from
    /// the given `ty::BoundRegion`, print it as "`'1`". Free regions that would ordinarily
    /// have names print as normal.
    ///
    /// This is used when you have a signature like `fn foo(x: &u32,
    /// y: &'a u32)` and we want to give a name to the region of the
    /// reference `x`.
    highlight_bound_region: Option<(ty::BoundRegion, usize)>,
}

thread_local! {
    /// Mechanism for highlighting of specific regions for display in NLL region inference errors.
    /// Contains region to highlight and counter for number to use when highlighting.
    static REGION_HIGHLIGHT_MODE: Cell<RegionHighlightMode> =
        Cell::new(RegionHighlightMode::default())
}

impl RegionHighlightMode {
    /// Reads and returns the current region highlight settings (accesses thread-local state).
    pub fn get() -> Self {
        REGION_HIGHLIGHT_MODE.with(|c| c.get())
    }

    // Internal helper to update current settings during the execution of `op`.
    fn set<R>(
        old_mode: Self,
        new_mode: Self,
        op: impl FnOnce() -> R,
    ) -> R {
        REGION_HIGHLIGHT_MODE.with(|c| {
            c.set(new_mode);
            let result = op();
            c.set(old_mode);
            result
        })
    }

    /// If `region` and `number` are both `Some`, invokes
    /// `highlighting_region`; otherwise, just invokes `op` directly.
    pub fn maybe_highlighting_region<R>(
        region: Option<ty::Region<'_>>,
        number: Option<usize>,
        op: impl FnOnce() -> R,
    ) -> R {
        if let Some(k) = region {
            if let Some(n) = number {
                return Self::highlighting_region(k, n, op);
            }
        }

        op()
    }

    /// During the execution of `op`, highlights the region inference
    /// variable `vid` as `'N`. We can only highlight one region `vid`
    /// at a time.
    pub fn highlighting_region<R>(
        region: ty::Region<'_>,
        number: usize,
        op: impl FnOnce() -> R,
    ) -> R {
        let old_mode = Self::get();
        let mut new_mode = old_mode;
        let first_avail_slot = new_mode.highlight_regions.iter_mut()
            .filter(|s| s.is_none())
            .next()
            .unwrap_or_else(|| {
                panic!(
                    "can only highlight {} placeholders at a time",
                    old_mode.highlight_regions.len(),
                )
            });
        *first_avail_slot = Some((*region, number));
        Self::set(old_mode, new_mode, op)
    }

    /// Convenience wrapper for `highlighting_region`.
    pub fn highlighting_region_vid<R>(
        vid: ty::RegionVid,
        number: usize,
        op: impl FnOnce() -> R,
    ) -> R {
        Self::highlighting_region(&ty::ReVar(vid), number, op)
    }

    /// Returns `true` if any placeholders are highlighted, and `false` otherwise.
    fn any_region_vids_highlighted(&self) -> bool {
        Self::get()
            .highlight_regions
            .iter()
            .any(|h| match h {
                Some((ty::ReVar(_), _)) => true,
                _ => false,
            })
    }

    /// Returns `Some(n)` with the number to use for the given region, if any.
    fn region_highlighted(&self, region: ty::Region<'_>) -> Option<usize> {
        Self::get()
            .highlight_regions
            .iter()
            .filter_map(|h| match h {
                Some((r, n)) if r == region => Some(*n),
                _ => None,
            })
            .next()
    }

    /// During the execution of `op`, highlight the given bound
    /// region. We can only highlight one bound region at a time. See
    /// the field `highlight_bound_region` for more detailed notes.
    pub fn highlighting_bound_region<R>(
        br: ty::BoundRegion,
        number: usize,
        op: impl FnOnce() -> R,
    ) -> R {
        let old_mode = Self::get();
        assert!(old_mode.highlight_bound_region.is_none());
        Self::set(
            old_mode,
            Self {
                highlight_bound_region: Some((br, number)),
                ..old_mode
            },
            op,
        )
    }

    /// Returns `true` if any placeholders are highlighted, and `false` otherwise.
    pub fn any_placeholders_highlighted(&self) -> bool {
        Self::get()
            .highlight_regions
            .iter()
            .any(|h| match h {
                Some((ty::RePlaceholder(_), _)) => true,
                _ => false,
            })
    }

    /// Returns `Some(N)` if the placeholder `p` is highlighted to print as "`'N`".
    pub fn placeholder_highlight(&self, p: ty::PlaceholderRegion) -> Option<usize> {
        self.region_highlighted(&ty::RePlaceholder(p))
    }
}

macro_rules! gen_display_debug_body {
    ( $with:path ) => {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            PrintCx::with(|mut cx| $with(self, f, &mut cx))
        }
    };
}
macro_rules! gen_display_debug {
    ( ($($x:tt)+) $target:ty, display yes ) => {
        impl<$($x)+> fmt::Display for $target {
            gen_display_debug_body! { Print::print_display }
        }
    };
    ( () $target:ty, display yes ) => {
        impl fmt::Display for $target {
            gen_display_debug_body! { Print::print_display }
        }
    };
    ( ($($x:tt)+) $target:ty, debug yes ) => {
        impl<$($x)+> fmt::Debug for $target {
            gen_display_debug_body! { Print::print_debug }
        }
    };
    ( () $target:ty, debug yes ) => {
        impl fmt::Debug for $target {
            gen_display_debug_body! { Print::print_debug }
        }
    };
    ( $generic:tt $target:ty, $t:ident no ) => {};
}
macro_rules! gen_print_impl {
    ( ($($x:tt)+) $target:ty, ($self:ident, $f:ident, $cx:ident) $disp:block $dbg:block ) => {
        impl<$($x)+> Print<'tcx> for $target {
            fn print<F: fmt::Write>(
                &$self,
                $f: &mut F,
                $cx: &mut PrintCx<'_, '_, '_>,
            ) -> fmt::Result {
                if $cx.is_debug $dbg
                else $disp
            }
        }
    };
    ( () $target:ty, ($self:ident, $f:ident, $cx:ident) $disp:block $dbg:block ) => {
        impl Print<'tcx> for $target {
            fn print<F: fmt::Write>(
                &$self,
                $f: &mut F,
                $cx: &mut PrintCx<'_, '_, '_>,
            ) -> fmt::Result {
                if $cx.is_debug $dbg
                else $disp
            }
        }
    };
    ( $generic:tt $target:ty,
      $vars:tt $gendisp:ident $disp:block $gendbg:ident $dbg:block ) => {
        gen_print_impl! { $generic $target, $vars $disp $dbg }
        gen_display_debug! { $generic $target, display $gendisp }
        gen_display_debug! { $generic $target, debug $gendbg }
    }
}
macro_rules! define_print {
    ( $generic:tt $target:ty,
      $vars:tt { display $disp:block debug $dbg:block } ) => {
        gen_print_impl! { $generic $target, $vars yes $disp yes $dbg }
    };
    ( $generic:tt $target:ty,
      $vars:tt { debug $dbg:block display $disp:block } ) => {
        gen_print_impl! { $generic $target, $vars yes $disp yes $dbg }
    };
    ( $generic:tt $target:ty,
      $vars:tt { debug $dbg:block } ) => {
        gen_print_impl! { $generic $target, $vars no {
            bug!(concat!("display not implemented for ", stringify!($target)));
        } yes $dbg }
    };
    ( $generic:tt $target:ty,
      ($self:ident, $f:ident, $cx:ident) { display $disp:block } ) => {
        gen_print_impl! { $generic $target, ($self, $f, $cx) yes $disp no {
            write!($f, "{:?}", $self)
        } }
    };
}
macro_rules! define_print_multi {
    ( [ $($generic:tt $target:ty),* ] $vars:tt $def:tt ) => {
        $(define_print! { $generic $target, $vars $def })*
    };
}
macro_rules! print_inner {
    ( $f:expr, $cx:expr, write ($($data:expr),+) ) => {
        write!($f, $($data),+)
    };
    ( $f:expr, $cx:expr, $kind:ident ($data:expr) ) => {
        $data.$kind($f, $cx)
    };
}
macro_rules! print {
    ( $f:expr, $cx:expr $(, $kind:ident $data:tt)+ ) => {
        Ok(())$(.and_then(|_| print_inner!($f, $cx, $kind $data)))+
    };
}

impl PrintCx<'a, 'gcx, 'tcx> {
    fn fn_sig<F: fmt::Write>(&mut self,
                             f: &mut F,
                             inputs: &[Ty<'_>],
                             c_variadic: bool,
                             output: Ty<'_>)
                             -> fmt::Result {
        write!(f, "(")?;
        let mut inputs = inputs.iter();
        if let Some(&ty) = inputs.next() {
            print!(f, self, print_display(ty))?;
            for &ty in inputs {
                print!(f, self, write(", "), print_display(ty))?;
            }
            if c_variadic {
                write!(f, ", ...")?;
            }
        }
        write!(f, ")")?;
        if !output.is_unit() {
            print!(f, self, write(" -> "), print_display(output))?;
        }

        Ok(())
    }

    fn parameterized<F: fmt::Write>(&mut self,
                                    f: &mut F,
                                    substs: SubstsRef<'_>,
                                    did: DefId,
                                    projections: &[ty::ProjectionPredicate<'_>])
                                    -> fmt::Result {
        let key = self.tcx.def_key(did);

        let verbose = self.is_verbose;
        let mut num_supplied_defaults = 0;
        let has_self;
        let mut own_counts: GenericParamCount = Default::default();
        let mut is_value_path = false;
        let mut item_name = Some(key.disambiguated_data.data.as_interned_str());
        let mut path_def_id = did;
        {
            // Unfortunately, some kinds of items (e.g., closures) don't have
            // generics. So walk back up the find the closest parent that DOES
            // have them.
            let mut item_def_id = did;
            loop {
                let key = self.tcx.def_key(item_def_id);
                match key.disambiguated_data.data {
                    DefPathData::AssocTypeInTrait(_) |
                    DefPathData::AssocTypeInImpl(_) |
                    DefPathData::AssocExistentialInImpl(_) |
                    DefPathData::Trait(_) |
                    DefPathData::TraitAlias(_) |
                    DefPathData::Impl |
                    DefPathData::TypeNs(_) => {
                        break;
                    }
                    DefPathData::ValueNs(_) |
                    DefPathData::EnumVariant(_) => {
                        is_value_path = true;
                        break;
                    }
                    DefPathData::CrateRoot |
                    DefPathData::Misc |
                    DefPathData::Module(_) |
                    DefPathData::MacroDef(_) |
                    DefPathData::ClosureExpr |
                    DefPathData::TypeParam(_) |
                    DefPathData::LifetimeParam(_) |
                    DefPathData::ConstParam(_) |
                    DefPathData::Field(_) |
                    DefPathData::StructCtor |
                    DefPathData::AnonConst |
                    DefPathData::ImplTrait |
                    DefPathData::GlobalMetaData(_) => {
                        // if we're making a symbol for something, there ought
                        // to be a value or type-def or something in there
                        // *somewhere*
                        item_def_id.index = key.parent.unwrap_or_else(|| {
                            bug!("finding type for {:?}, encountered def-id {:?} with no \
                                 parent", did, item_def_id);
                        });
                    }
                }
            }
            let mut generics = self.tcx.generics_of(item_def_id);
            let child_own_counts = generics.own_counts();
            has_self = generics.has_self;

            let mut child_types = 0;
            if let Some(def_id) = generics.parent {
                // Methods.
                assert!(is_value_path);
                child_types = child_own_counts.types;
                generics = self.tcx.generics_of(def_id);
                own_counts = generics.own_counts();

                if has_self {
                    print!(f, self, write("<"), print_display(substs.type_at(0)), write(" as "))?;
                }

                path_def_id = def_id;
            } else {
                item_name = None;

                if is_value_path {
                    // Functions.
                    assert_eq!(has_self, false);
                } else {
                    // Types and traits.
                    own_counts = child_own_counts;
                }
            }

            if !verbose {
                let mut type_params =
                    generics.params.iter().rev().filter_map(|param| match param.kind {
                        GenericParamDefKind::Lifetime => None,
                        GenericParamDefKind::Type { has_default, .. } => {
                            Some((param.def_id, has_default))
                        }
                        GenericParamDefKind::Const => None, // FIXME(const_generics:defaults)
                    }).peekable();
                let has_default = {
                    let has_default = type_params.peek().map(|(_, has_default)| has_default);
                    *has_default.unwrap_or(&false)
                };
                if has_default {
                    let substs = self.tcx.lift(&substs).expect("could not lift for printing");
                    let types = substs.types().rev().skip(child_types);
                    for ((def_id, has_default), actual) in type_params.zip(types) {
                        if !has_default {
                            break;
                        }
                        if self.tcx.type_of(def_id).subst(self.tcx, substs) != actual {
                            break;
                        }
                        num_supplied_defaults += 1;
                    }
                }
            }
        }
        print!(f, self, write("{}", self.tcx.item_path_str(path_def_id)))?;
        let fn_trait_kind = self.tcx.lang_items().fn_trait_kind(path_def_id);

        if !verbose && fn_trait_kind.is_some() && projections.len() == 1 {
            let projection_ty = projections[0].ty;
            if let Tuple(ref args) = substs.type_at(1).sty {
                return self.fn_sig(f, args, false, projection_ty);
            }
        }

        let empty = Cell::new(true);
        let start_or_continue = |f: &mut F, start: &str, cont: &str| {
            if empty.get() {
                empty.set(false);
                write!(f, "{}", start)
            } else {
                write!(f, "{}", cont)
            }
        };

        let print_regions = |f: &mut F, start: &str, skip, count| {
            // Don't print any regions if they're all erased.
            let regions = || substs.regions().skip(skip).take(count);
            if regions().all(|r: ty::Region<'_>| *r == ty::ReErased) {
                return Ok(());
            }

            for region in regions() {
                let region: ty::Region<'_> = region;
                start_or_continue(f, start, ", ")?;
                if verbose {
                    write!(f, "{:?}", region)?;
                } else {
                    let s = region.to_string();
                    if s.is_empty() {
                        // This happens when the value of the region
                        // parameter is not easily serialized. This may be
                        // because the user omitted it in the first place,
                        // or because it refers to some block in the code,
                        // etc. I'm not sure how best to serialize this.
                        write!(f, "'_")?;
                    } else {
                        write!(f, "{}", s)?;
                    }
                }
            }

            Ok(())
        };

        print_regions(f, "<", 0, own_counts.lifetimes)?;

        let tps = substs.types()
                        .take(own_counts.types - num_supplied_defaults)
                        .skip(has_self as usize);

        for ty in tps {
            start_or_continue(f, "<", ", ")?;
            ty.print_display(f, self)?;
        }

        for projection in projections {
            start_or_continue(f, "<", ", ")?;
            print!(f, self,
                    write("{}=",
                            self.tcx.associated_item(projection.projection_ty.item_def_id).ident),
                    print_display(projection.ty))?;
        }

        // FIXME(const_generics::defaults)
        let consts = substs.consts();

        for ct in consts {
            start_or_continue(f, "<", ", ")?;
            ct.print_display(f, self)?;
        }

        start_or_continue(f, "", ">")?;

        // For values, also print their name and type parameters.
        if is_value_path {
            empty.set(true);

            if has_self {
                write!(f, ">")?;
            }

            if let Some(item_name) = item_name {
                write!(f, "::{}", item_name)?;
            }

            print_regions(f, "::<", own_counts.lifetimes, usize::MAX)?;

            // FIXME: consider being smart with defaults here too
            for ty in substs.types().skip(own_counts.types) {
                start_or_continue(f, "::<", ", ")?;
                ty.print_display(f, self)?;
            }

            start_or_continue(f, "", ">")?;
        }

        Ok(())
    }

    fn in_binder<T, F>(&mut self, f: &mut F, value: ty::Binder<T>) -> fmt::Result
        where T: Print<'tcx> + TypeFoldable<'tcx>, F: fmt::Write
    {
        fn name_by_region_index(index: usize) -> InternedString {
            match index {
                0 => Symbol::intern("'r"),
                1 => Symbol::intern("'s"),
                i => Symbol::intern(&format!("'t{}", i-2)),
            }.as_interned_str()
        }

        // Replace any anonymous late-bound regions with named
        // variants, using gensym'd identifiers, so that we can
        // clearly differentiate between named and unnamed regions in
        // the output. We'll probably want to tweak this over time to
        // decide just how much information to give.
        if self.binder_depth == 0 {
            self.prepare_late_bound_region_info(&value);
        }

        let mut empty = true;
        let mut start_or_continue = |f: &mut F, start: &str, cont: &str| {
            if empty {
                empty = false;
                write!(f, "{}", start)
            } else {
                write!(f, "{}", cont)
            }
        };

        let old_region_index = self.region_index;
        let mut region_index = old_region_index;
        let new_value = self.tcx.replace_late_bound_regions(&value, |br| {
            let _ = start_or_continue(f, "for<", ", ");
            let br = match br {
                ty::BrNamed(_, name) => {
                    let _ = write!(f, "{}", name);
                    br
                }
                ty::BrAnon(_) |
                ty::BrFresh(_) |
                ty::BrEnv => {
                    let name = loop {
                        let name = name_by_region_index(region_index);
                        region_index += 1;
                        if !self.is_name_used(&name) {
                            break name;
                        }
                    };
                    let _ = write!(f, "{}", name);
                    ty::BrNamed(self.tcx.hir().local_def_id(CRATE_NODE_ID), name)
                }
            };
            self.tcx.mk_region(ty::ReLateBound(ty::INNERMOST, br))
        }).0;
        start_or_continue(f, "", "> ")?;

        // Push current state to gcx, and restore after writing new_value.
        self.binder_depth += 1;
        self.region_index = region_index;
        let result = new_value.print_display(f, self);
        self.region_index = old_region_index;
        self.binder_depth -= 1;
        result
    }

    fn is_name_used(&self, name: &InternedString) -> bool {
        match self.used_region_names {
            Some(ref names) => names.contains(name),
            None => false,
        }
    }
}

pub fn parameterized<F: fmt::Write>(f: &mut F,
                                    substs: SubstsRef<'_>,
                                    did: DefId,
                                    projections: &[ty::ProjectionPredicate<'_>])
                                    -> fmt::Result {
    PrintCx::with(|mut cx| cx.parameterized(f, substs, did, projections))
}

impl<'a, 'tcx, T: Print<'tcx>> Print<'tcx> for &'a T {
    fn print<F: fmt::Write>(&self, f: &mut F, cx: &mut PrintCx<'_, '_, '_>) -> fmt::Result {
        (*self).print(f, cx)
    }
}

define_print! {
    ('tcx) &'tcx ty::List<ty::ExistentialPredicate<'tcx>>, (self, f, cx) {
        display {
            // Generate the main trait ref, including associated types.

            // Use a type that can't appear in defaults of type parameters.
            let dummy_self = cx.tcx.mk_infer(ty::FreshTy(0));
            let mut first = true;

            if let Some(principal) = self.principal() {
                let principal = cx.tcx
                    .lift(&principal)
                    .expect("could not lift for printing")
                    .with_self_ty(cx.tcx, dummy_self);
                let projections = self.projection_bounds().map(|p| {
                    cx.tcx.lift(&p)
                        .expect("could not lift for printing")
                        .with_self_ty(cx.tcx, dummy_self)
                }).collect::<Vec<_>>();
                cx.parameterized(f, principal.substs, principal.def_id, &projections)?;
                first = false;
            }

            // Builtin bounds.
            let mut auto_traits: Vec<_> = self.auto_traits().map(|did| {
                cx.tcx.item_path_str(did)
            }).collect();

            // The auto traits come ordered by `DefPathHash`. While
            // `DefPathHash` is *stable* in the sense that it depends on
            // neither the host nor the phase of the moon, it depends
            // "pseudorandomly" on the compiler version and the target.
            //
            // To avoid that causing instabilities in compiletest
            // output, sort the auto-traits alphabetically.
            auto_traits.sort();

            for auto_trait in auto_traits {
                if !first {
                    write!(f, " + ")?;
                }
                first = false;

                write!(f, "{}", auto_trait)?;
            }

            Ok(())
        }
    }
}

impl fmt::Debug for ty::GenericParamDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = match self.kind {
            ty::GenericParamDefKind::Lifetime => "Lifetime",
            ty::GenericParamDefKind::Type { .. } => "Type",
            ty::GenericParamDefKind::Const => "Const",
        };
        write!(f, "{}({}, {:?}, {})",
               type_name,
               self.name,
               self.def_id,
               self.index)
    }
}

impl fmt::Debug for ty::TraitDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        PrintCx::with(|cx| {
            write!(f, "{}", cx.tcx.item_path_str(self.def_id))
        })
    }
}

impl fmt::Debug for ty::AdtDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        PrintCx::with(|cx| {
            write!(f, "{}", cx.tcx.item_path_str(self.did))
        })
    }
}

impl<'tcx> fmt::Debug for ty::ClosureUpvar<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClosureUpvar({:?},{:?})",
               self.def,
               self.ty)
    }
}

impl fmt::Debug for ty::UpvarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UpvarId({:?};`{}`;{:?})",
               self.var_path.hir_id,
               PrintCx::with(|cx| {
                    cx.tcx.hir().name_by_hir_id(self.var_path.hir_id)
               }),
               self.closure_expr_id)
    }
}

impl<'tcx> fmt::Debug for ty::UpvarBorrow<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UpvarBorrow({:?}, {:?})",
               self.kind, self.region)
    }
}

define_print! {
    ('tcx) &'tcx ty::List<Ty<'tcx>>, (self, f, cx) {
        display {
            write!(f, "{{")?;
            let mut tys = self.iter();
            if let Some(&ty) = tys.next() {
                print!(f, cx, print(ty))?;
                for &ty in tys {
                    print!(f, cx, write(", "), print(ty))?;
                }
            }
            write!(f, "}}")
        }
    }
}

define_print! {
    ('tcx) ty::TypeAndMut<'tcx>, (self, f, cx) {
        display {
            print!(f, cx,
                   write("{}", if self.mutbl == hir::MutMutable { "mut " } else { "" }),
                   print(self.ty))
        }
    }
}

define_print! {
    ('tcx) ty::ExistentialTraitRef<'tcx>, (self, f, cx) {
        display {
            cx.parameterized(f, self.substs, self.def_id, &[])
        }
        debug {
            let dummy_self = cx.tcx.mk_infer(ty::FreshTy(0));

            let trait_ref = *cx.tcx.lift(&ty::Binder::bind(*self))
                                .expect("could not lift for printing")
                                .with_self_ty(cx.tcx, dummy_self).skip_binder();
            cx.parameterized(f, trait_ref.substs, trait_ref.def_id, &[])
        }
    }
}

define_print! {
    ('tcx) ty::adjustment::Adjustment<'tcx>, (self, f, cx) {
        debug {
            print!(f, cx, write("{:?} -> ", self.kind), print(self.target))
        }
    }
}

define_print! {
    () ty::BoundRegion, (self, f, cx) {
        display {
            if cx.is_verbose {
                return self.print_debug(f, cx);
            }

            if let Some((region, counter)) = RegionHighlightMode::get().highlight_bound_region {
                if *self == region {
                    return match *self {
                        BrNamed(_, name) => write!(f, "{}", name),
                        BrAnon(_) | BrFresh(_) | BrEnv => write!(f, "'{}", counter)
                    };
                }
            }

            match *self {
                BrNamed(_, name) => write!(f, "{}", name),
                BrAnon(_) | BrFresh(_) | BrEnv => Ok(())
            }
        }
        debug {
            return match *self {
                BrAnon(n) => write!(f, "BrAnon({:?})", n),
                BrFresh(n) => write!(f, "BrFresh({:?})", n),
                BrNamed(did, name) => {
                    write!(f, "BrNamed({:?}:{:?}, {})",
                           did.krate, did.index, name)
                }
                BrEnv => write!(f, "BrEnv"),
            };
        }
    }
}

define_print! {
    () ty::PlaceholderRegion, (self, f, cx) {
        display {
            if cx.is_verbose {
                return self.print_debug(f, cx);
            }

            let highlight = RegionHighlightMode::get();
            if let Some(counter) = highlight.placeholder_highlight(*self) {
                write!(f, "'{}", counter)
            } else if highlight.any_placeholders_highlighted() {
                write!(f, "'_")
            } else {
                write!(f, "{}", self.name)
            }
        }
    }
}

define_print! {
    () ty::RegionKind, (self, f, cx) {
        display {
            if cx.is_verbose {
                return self.print_debug(f, cx);
            }

            // Watch out for region highlights.
            if let Some(n) = RegionHighlightMode::get().region_highlighted(self) {
                return write!(f, "'{:?}", n);
            }

            // These printouts are concise.  They do not contain all the information
            // the user might want to diagnose an error, but there is basically no way
            // to fit that into a short string.  Hence the recommendation to use
            // `explain_region()` or `note_and_explain_region()`.
            match *self {
                ty::ReEarlyBound(ref data) => {
                    write!(f, "{}", data.name)
                }
                ty::ReLateBound(_, br) |
                ty::ReFree(ty::FreeRegion { bound_region: br, .. }) => {
                    write!(f, "{}", br)
                }
                ty::RePlaceholder(p) => {
                    write!(f, "{}", p)
                }
                ty::ReScope(scope) if cx.identify_regions => {
                    match scope.data {
                        region::ScopeData::Node =>
                            write!(f, "'{}s", scope.item_local_id().as_usize()),
                        region::ScopeData::CallSite =>
                            write!(f, "'{}cs", scope.item_local_id().as_usize()),
                        region::ScopeData::Arguments =>
                            write!(f, "'{}as", scope.item_local_id().as_usize()),
                        region::ScopeData::Destruction =>
                            write!(f, "'{}ds", scope.item_local_id().as_usize()),
                        region::ScopeData::Remainder(first_statement_index) => write!(
                            f,
                            "'{}_{}rs",
                            scope.item_local_id().as_usize(),
                            first_statement_index.index()
                        ),
                    }
                }
                ty::ReVar(region_vid) => {
                    if RegionHighlightMode::get().any_region_vids_highlighted() {
                        write!(f, "{:?}", region_vid)
                    } else if cx.identify_regions {
                        write!(f, "'{}rv", region_vid.index())
                    } else {
                        Ok(())
                    }
                }
                ty::ReScope(_) |
                ty::ReErased => Ok(()),
                ty::ReStatic => write!(f, "'static"),
                ty::ReEmpty => write!(f, "'<empty>"),

                // The user should never encounter these in unsubstituted form.
                ty::ReClosureBound(vid) => write!(f, "{:?}", vid),
            }
        }
        debug {
            match *self {
                ty::ReEarlyBound(ref data) => {
                    write!(f, "ReEarlyBound({}, {})",
                           data.index,
                           data.name)
                }

                ty::ReClosureBound(ref vid) => {
                    write!(f, "ReClosureBound({:?})",
                           vid)
                }

                ty::ReLateBound(binder_id, ref bound_region) => {
                    write!(f, "ReLateBound({:?}, {:?})",
                           binder_id,
                           bound_region)
                }

                ty::ReFree(ref fr) => write!(f, "{:?}", fr),

                ty::ReScope(id) => {
                    write!(f, "ReScope({:?})", id)
                }

                ty::ReStatic => write!(f, "ReStatic"),

                ty::ReVar(ref vid) => {
                    write!(f, "{:?}", vid)
                }

                ty::RePlaceholder(placeholder) => {
                    write!(f, "RePlaceholder({:?})", placeholder)
                }

                ty::ReEmpty => write!(f, "ReEmpty"),

                ty::ReErased => write!(f, "ReErased")
            }
        }
    }
}

define_print! {
    () ty::FreeRegion, (self, f, cx) {
        debug {
            write!(f, "ReFree({:?}, {:?})", self.scope, self.bound_region)
        }
    }
}

define_print! {
    () ty::Variance, (self, f, cx) {
        debug {
            f.write_str(match *self {
                ty::Covariant => "+",
                ty::Contravariant => "-",
                ty::Invariant => "o",
                ty::Bivariant => "*",
            })
        }
    }
}

define_print! {
    ('tcx) ty::GenericPredicates<'tcx>, (self, f, cx) {
        debug {
            write!(f, "GenericPredicates({:?})", self.predicates)
        }
    }
}

define_print! {
    ('tcx) ty::InstantiatedPredicates<'tcx>, (self, f, cx) {
        debug {
            write!(f, "InstantiatedPredicates({:?})", self.predicates)
        }
    }
}

define_print! {
    ('tcx) ty::FnSig<'tcx>, (self, f, cx) {
        display {
            if self.unsafety == hir::Unsafety::Unsafe {
                write!(f, "unsafe ")?;
            }

            if self.abi != Abi::Rust {
                write!(f, "extern {} ", self.abi)?;
            }

            write!(f, "fn")?;
            cx.fn_sig(f, self.inputs(), self.c_variadic, self.output())
        }
        debug {
            write!(f, "({:?}; c_variadic: {})->{:?}", self.inputs(), self.c_variadic, self.output())
        }
    }
}

impl fmt::Debug for ty::TyVid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_#{}t", self.index)
    }
}

impl<'tcx> fmt::Debug for ty::ConstVid<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_#{}f", self.index)
    }
}

impl fmt::Debug for ty::IntVid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_#{}i", self.index)
    }
}

impl fmt::Debug for ty::FloatVid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_#{}f", self.index)
    }
}

impl fmt::Debug for ty::RegionVid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(counter) = RegionHighlightMode::get().region_highlighted(&ty::ReVar(*self)) {
            return write!(f, "'{:?}", counter);
        } else if RegionHighlightMode::get().any_region_vids_highlighted() {
            return write!(f, "'_");
        }

        write!(f, "'_#{}r", self.index())
    }
}

define_print! {
    () ty::InferTy, (self, f, cx) {
        display {
            if cx.is_verbose {
                print!(f, cx, print_debug(self))
            } else {
                match *self {
                    ty::TyVar(_) => write!(f, "_"),
                    ty::IntVar(_) => write!(f, "{}", "{integer}"),
                    ty::FloatVar(_) => write!(f, "{}", "{float}"),
                    ty::FreshTy(v) => write!(f, "FreshTy({})", v),
                    ty::FreshIntTy(v) => write!(f, "FreshIntTy({})", v),
                    ty::FreshFloatTy(v) => write!(f, "FreshFloatTy({})", v)
                }
            }
        }
        debug {
            match *self {
                ty::TyVar(ref v) => write!(f, "{:?}", v),
                ty::IntVar(ref v) => write!(f, "{:?}", v),
                ty::FloatVar(ref v) => write!(f, "{:?}", v),
                ty::FreshTy(v) => write!(f, "FreshTy({:?})", v),
                ty::FreshIntTy(v) => write!(f, "FreshIntTy({:?})", v),
                ty::FreshFloatTy(v) => write!(f, "FreshFloatTy({:?})", v)
            }
        }
    }
}

impl fmt::Debug for ty::IntVarValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ty::IntType(ref v) => v.fmt(f),
            ty::UintType(ref v) => v.fmt(f),
        }
    }
}

impl fmt::Debug for ty::FloatVarValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// The generic impl doesn't work yet because projections are not
// normalized under HRTB.
/*impl<T> fmt::Display for ty::Binder<T>
    where T: fmt::Display + for<'a> ty::Lift<'a>,
          for<'a> <T as ty::Lift<'a>>::Lifted: fmt::Display + TypeFoldable<'a>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        PrintCx::with(|cx| cx.in_binder(f, cx.tcx.lift(self)
            .expect("could not lift for printing")))
    }
}*/

define_print_multi! {
    [
    ('tcx) ty::Binder<&'tcx ty::List<ty::ExistentialPredicate<'tcx>>>,
    ('tcx) ty::Binder<ty::TraitRef<'tcx>>,
    ('tcx) ty::Binder<ty::FnSig<'tcx>>,
    ('tcx) ty::Binder<ty::TraitPredicate<'tcx>>,
    ('tcx) ty::Binder<ty::SubtypePredicate<'tcx>>,
    ('tcx) ty::Binder<ty::ProjectionPredicate<'tcx>>,
    ('tcx) ty::Binder<ty::OutlivesPredicate<Ty<'tcx>, ty::Region<'tcx>>>,
    ('tcx) ty::Binder<ty::OutlivesPredicate<ty::Region<'tcx>, ty::Region<'tcx>>>
    ]
    (self, f, cx) {
        display {
            cx.in_binder(f, cx.tcx.lift(self)
                .expect("could not lift for printing"))
        }
    }
}

define_print! {
    ('tcx) ty::TraitRef<'tcx>, (self, f, cx) {
        display {
            cx.parameterized(f, self.substs, self.def_id, &[])
        }
        debug {
            // when printing out the debug representation, we don't need
            // to enumerate the `for<...>` etc because the debruijn index
            // tells you everything you need to know.
            print!(f, cx,
                   write("<"),
                   print(self.self_ty()),
                   write(" as "))?;
            cx.parameterized(f, self.substs, self.def_id, &[])?;
            write!(f, ">")
        }
    }
}

define_print! {
    ('tcx) ty::Ty<'tcx>, (self, f, cx) {
        display {
            match self.sty {
                Bool => write!(f, "bool"),
                Char => write!(f, "char"),
                Int(t) => write!(f, "{}", t.ty_to_string()),
                Uint(t) => write!(f, "{}", t.ty_to_string()),
                Float(t) => write!(f, "{}", t.ty_to_string()),
                RawPtr(ref tm) => {
                    write!(f, "*{} ", match tm.mutbl {
                        hir::MutMutable => "mut",
                        hir::MutImmutable => "const",
                    })?;
                    tm.ty.print(f, cx)
                }
                Ref(r, ty, mutbl) => {
                    write!(f, "&")?;
                    let s = r.print_to_string(cx);
                    if s != "'_" {
                        write!(f, "{}", s)?;
                        if !s.is_empty() {
                            write!(f, " ")?;
                        }
                    }
                    ty::TypeAndMut { ty, mutbl }.print(f, cx)
                }
                Never => write!(f, "!"),
                Tuple(ref tys) => {
                    write!(f, "(")?;
                    let mut tys = tys.iter();
                    if let Some(&ty) = tys.next() {
                        print!(f, cx, print(ty), write(","))?;
                        if let Some(&ty) = tys.next() {
                            print!(f, cx, write(" "), print(ty))?;
                            for &ty in tys {
                                print!(f, cx, write(", "), print(ty))?;
                            }
                        }
                    }
                    write!(f, ")")
                }
                FnDef(def_id, substs) => {
                    let substs = cx.tcx.lift(&substs)
                        .expect("could not lift for printing");
                    let sig = cx.tcx.fn_sig(def_id).subst(cx.tcx, substs);
                    print!(f, cx, print(sig), write(" {{"))?;
                    cx.parameterized(f, substs, def_id, &[])?;
                    write!(f, "}}")
                }
                FnPtr(ref bare_fn) => {
                    bare_fn.print(f, cx)
                }
                Infer(infer_ty) => write!(f, "{}", infer_ty),
                Error => write!(f, "[type error]"),
                Param(ref param_ty) => write!(f, "{}", param_ty),
                Bound(debruijn, bound_ty) => {
                    match bound_ty.kind {
                        ty::BoundTyKind::Anon => {
                            if debruijn == ty::INNERMOST {
                                write!(f, "^{}", bound_ty.var.index())
                            } else {
                                write!(f, "^{}_{}", debruijn.index(), bound_ty.var.index())
                            }
                        }

                        ty::BoundTyKind::Param(p) => write!(f, "{}", p),
                    }
                }
                Adt(def, substs) => cx.parameterized(f, substs, def.did, &[]),
                Dynamic(data, r) => {
                    let r = r.print_to_string(cx);
                    if !r.is_empty() {
                        write!(f, "(")?;
                    }
                    write!(f, "dyn ")?;
                    data.print(f, cx)?;
                    if !r.is_empty() {
                        write!(f, " + {})", r)
                    } else {
                        Ok(())
                    }
                }
                Foreign(def_id) => parameterized(f, subst::InternalSubsts::empty(), def_id, &[]),
                Projection(ref data) => data.print(f, cx),
                UnnormalizedProjection(ref data) => {
                    write!(f, "Unnormalized(")?;
                    data.print(f, cx)?;
                    write!(f, ")")
                }
                Placeholder(placeholder) => {
                    write!(f, "Placeholder({:?})", placeholder)
                }
                Opaque(def_id, substs) => {
                    if cx.is_verbose {
                        return write!(f, "Opaque({:?}, {:?})", def_id, substs);
                    }

                    let def_key = cx.tcx.def_key(def_id);
                    if let Some(name) = def_key.disambiguated_data.data.get_opt_name() {
                        write!(f, "{}", name)?;
                        let mut substs = substs.iter();
                        if let Some(first) = substs.next() {
                            write!(f, "::<")?;
                            write!(f, "{}", first)?;
                            for subst in substs {
                                write!(f, ", {}", subst)?;
                            }
                            write!(f, ">")?;
                        }
                        return Ok(());
                    }
                    // Grab the "TraitA + TraitB" from `impl TraitA + TraitB`,
                    // by looking up the projections associated with the def_id.
                    let substs = cx.tcx.lift(&substs)
                        .expect("could not lift for printing");
                    let bounds = cx.tcx.predicates_of(def_id).instantiate(cx.tcx, substs);

                    let mut first = true;
                    let mut is_sized = false;
                    write!(f, "impl")?;
                    for predicate in bounds.predicates {
                        if let Some(trait_ref) = predicate.to_opt_poly_trait_ref() {
                            // Don't print +Sized, but rather +?Sized if absent.
                            if Some(trait_ref.def_id()) == cx.tcx.lang_items().sized_trait() {
                                is_sized = true;
                                continue;
                            }

                            print!(f, cx,
                                    write("{}", if first { " " } else { "+" }),
                                    print(trait_ref))?;
                            first = false;
                        }
                    }
                    if !is_sized {
                        write!(f, "{}?Sized", if first { " " } else { "+" })?;
                        } else if first {
                            write!(f, " Sized")?;
                    }
                    Ok(())
                }
                Str => write!(f, "str"),
                Generator(did, substs, movability) => {
                    let upvar_tys = substs.upvar_tys(did, cx.tcx);
                    let witness = substs.witness(did, cx.tcx);
                    if movability == hir::GeneratorMovability::Movable {
                        write!(f, "[generator")?;
                    } else {
                        write!(f, "[static generator")?;
                    }

                    if let Some(hir_id) = cx.tcx.hir().as_local_hir_id(did) {
                        write!(f, "@{:?}", cx.tcx.hir().span_by_hir_id(hir_id))?;
                        let mut sep = " ";
                        cx.tcx.with_freevars(hir_id, |freevars| {
                            for (freevar, upvar_ty) in freevars.iter().zip(upvar_tys) {
                                print!(f, cx,
                                       write("{}{}:",
                                             sep,
                                             cx.tcx.hir().name(freevar.var_id())),
                                       print(upvar_ty))?;
                                sep = ", ";
                            }
                            Ok(())
                        })?
                    } else {
                        // cross-crate closure types should only be
                        // visible in codegen bug reports, I imagine.
                        write!(f, "@{:?}", did)?;
                        let mut sep = " ";
                        for (index, upvar_ty) in upvar_tys.enumerate() {
                            print!(f, cx,
                                   write("{}{}:", sep, index),
                                   print(upvar_ty))?;
                            sep = ", ";
                        }
                    }

                    print!(f, cx, write(" "), print(witness), write("]"))
                },
                GeneratorWitness(types) => {
                    cx.in_binder(f, cx.tcx.lift(&types)
                        .expect("could not lift for printing"))
                }
                Closure(did, substs) => {
                    let upvar_tys = substs.upvar_tys(did, cx.tcx);
                    write!(f, "[closure")?;

                    if let Some(hir_id) = cx.tcx.hir().as_local_hir_id(did) {
                        if cx.tcx.sess.opts.debugging_opts.span_free_formats {
                            write!(f, "@{:?}", hir_id)?;
                        } else {
                            write!(f, "@{:?}", cx.tcx.hir().span_by_hir_id(hir_id))?;
                        }
                        let mut sep = " ";
                        cx.tcx.with_freevars(hir_id, |freevars| {
                            for (freevar, upvar_ty) in freevars.iter().zip(upvar_tys) {
                                print!(f, cx,
                                       write("{}{}:",
                                             sep,
                                             cx.tcx.hir().name(freevar.var_id())),
                                       print(upvar_ty))?;
                                sep = ", ";
                            }
                            Ok(())
                        })?
                    } else {
                        // cross-crate closure types should only be
                        // visible in codegen bug reports, I imagine.
                        write!(f, "@{:?}", did)?;
                        let mut sep = " ";
                        for (index, upvar_ty) in upvar_tys.enumerate() {
                            print!(f, cx,
                                   write("{}{}:", sep, index),
                                   print(upvar_ty))?;
                            sep = ", ";
                        }
                    }

                    if cx.is_verbose {
                        write!(
                            f,
                            " closure_kind_ty={:?} closure_sig_ty={:?}",
                            substs.closure_kind_ty(did, tcx),
                            substs.closure_sig_ty(did, tcx),
                        )?;
                    }

                    write!(f, "]")
                },
                Array(ty, sz) => {
                    print!(f, cx, write("["), print(ty), write("; "))?;
                    match sz {
                        ty::LazyConst::Unevaluated(_def_id, _substs) => {
                            write!(f, "_")?;
                        }
                        ty::LazyConst::Evaluated(c) => {
                            match c.val {
                                ConstValue::Infer(..) => write!(f, "_")?,
                                ConstValue::Param(ParamConst { name, .. }) =>
                                    write!(f, "{}", name)?,
                                _ => write!(f, "{}", c.unwrap_usize(cx.tcx))?,
                            }
                        }
                    }
                    write!(f, "]")
                }
                Slice(ty) => {
                    print!(f, cx, write("["), print(ty), write("]"))
                }
            }
        }
        debug {
            self.print_display(f, cx)
        }
    }
}

define_print! {
    ('tcx) ConstValue<'tcx>, (self, f, cx) {
        display {
            match self {
                ConstValue::Infer(..) => write!(f, "_"),
                ConstValue::Param(ParamConst { name, .. }) => write!(f, "{}", name),
                _ => write!(f, "{:?}", self),
            }
        }
    }
}

define_print! {
    ('tcx) ty::Const<'tcx>, (self, f, cx) {
        display {
            write!(f, "{} : {}", self.val, self.ty)
        }
    }
}

define_print! {
    ('tcx) ty::LazyConst<'tcx>, (self, f, cx) {
        display {
            match self {
                ty::LazyConst::Unevaluated(..) => write!(f, "_ : _"),
                ty::LazyConst::Evaluated(c) => write!(f, "{}", c),
            }
        }
    }
}

define_print! {
    () ty::ParamTy, (self, f, cx) {
        display {
            write!(f, "{}", self.name)
        }
        debug {
            write!(f, "{}/#{}", self.name, self.idx)
        }
    }
}

define_print! {
    () ty::ParamConst, (self, f, cx) {
        display {
            write!(f, "{}", self.name)
        }
        debug {
            write!(f, "{}/#{}", self.name, self.index)
        }
    }
}

define_print! {
    ('tcx, T: Print<'tcx> + fmt::Debug, U: Print<'tcx> + fmt::Debug) ty::OutlivesPredicate<T, U>,
    (self, f, cx) {
        display {
            print!(f, cx, print(self.0), write(" : "), print(self.1))
        }
    }
}

define_print! {
    ('tcx) ty::SubtypePredicate<'tcx>, (self, f, cx) {
        display {
            print!(f, cx, print(self.a), write(" <: "), print(self.b))
        }
    }
}

define_print! {
    ('tcx) ty::TraitPredicate<'tcx>, (self, f, cx) {
        debug {
            write!(f, "TraitPredicate({:?})",
                   self.trait_ref)
        }
        display {
            print!(f, cx, print(self.trait_ref.self_ty()), write(": "), print(self.trait_ref))
        }
    }
}

define_print! {
    ('tcx) ty::ProjectionPredicate<'tcx>, (self, f, cx) {
        debug {
            print!(f, cx,
                   write("ProjectionPredicate("),
                   print(self.projection_ty),
                   write(", "),
                   print(self.ty),
                   write(")"))
        }
        display {
            print!(f, cx, print(self.projection_ty), write(" == "), print(self.ty))
        }
    }
}

define_print! {
    ('tcx) ty::ProjectionTy<'tcx>, (self, f, cx) {
        display {
            // FIXME(tschottdorf): use something like
            //   parameterized(f, self.substs, self.item_def_id, &[])
            // (which currently ICEs).
            let trait_ref = self.trait_ref(cx.tcx);
            let item_name = cx.tcx.associated_item(self.item_def_id).ident;
            print!(f, cx, print_debug(trait_ref), write("::{}", item_name))
        }
    }
}

define_print! {
    () ty::ClosureKind, (self, f, cx) {
        display {
            match *self {
                ty::ClosureKind::Fn => write!(f, "Fn"),
                ty::ClosureKind::FnMut => write!(f, "FnMut"),
                ty::ClosureKind::FnOnce => write!(f, "FnOnce"),
            }
        }
    }
}

define_print! {
    ('tcx) ty::Predicate<'tcx>, (self, f, cx) {
        display {
            match *self {
                ty::Predicate::Trait(ref data) => data.print(f, cx),
                ty::Predicate::Subtype(ref predicate) => predicate.print(f, cx),
                ty::Predicate::RegionOutlives(ref predicate) => predicate.print(f, cx),
                ty::Predicate::TypeOutlives(ref predicate) => predicate.print(f, cx),
                ty::Predicate::Projection(ref predicate) => predicate.print(f, cx),
                ty::Predicate::WellFormed(ty) => print!(f, cx, print(ty), write(" well-formed")),
                ty::Predicate::ObjectSafe(trait_def_id) => {
                    write!(f, "the trait `{}` is object-safe", cx.tcx.item_path_str(trait_def_id))
                }
                ty::Predicate::ClosureKind(closure_def_id, _closure_substs, kind) => {
                    write!(f, "the closure `{}` implements the trait `{}`",
                           cx.tcx.item_path_str(closure_def_id), kind)
                }
                ty::Predicate::ConstEvaluatable(def_id, substs) => {
                    write!(f, "the constant `")?;
                    cx.parameterized(f, substs, def_id, &[])?;
                    write!(f, "` can be evaluated")
                }
            }
        }
        debug {
            match *self {
                ty::Predicate::Trait(ref a) => a.print(f, cx),
                ty::Predicate::Subtype(ref pair) => pair.print(f, cx),
                ty::Predicate::RegionOutlives(ref pair) => pair.print(f, cx),
                ty::Predicate::TypeOutlives(ref pair) => pair.print(f, cx),
                ty::Predicate::Projection(ref pair) => pair.print(f, cx),
                ty::Predicate::WellFormed(ty) => ty.print(f, cx),
                ty::Predicate::ObjectSafe(trait_def_id) => {
                    write!(f, "ObjectSafe({:?})", trait_def_id)
                }
                ty::Predicate::ClosureKind(closure_def_id, closure_substs, kind) => {
                    write!(f, "ClosureKind({:?}, {:?}, {:?})", closure_def_id, closure_substs, kind)
                }
                ty::Predicate::ConstEvaluatable(def_id, substs) => {
                    write!(f, "ConstEvaluatable({:?}, {:?})", def_id, substs)
                }
            }
        }
    }
}
