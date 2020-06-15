use crate::ty::{self, AdtSizedConstraint, Ty, TyCtxt, TyS};

use rustc_span::symbol::Symbol;

pub(super) trait Value<'tcx>: Sized {
    fn from_cycle_error(tcx: TyCtxt<'tcx>) -> Self;
}

impl<'tcx, T> Value<'tcx> for T {
    default fn from_cycle_error(tcx: TyCtxt<'tcx>) -> T {
        tcx.sess.abort_if_errors();
        bug!("Value::from_cycle_error called without errors");
    }
}

impl<'tcx> Value<'tcx> for &'_ TyS<'_> {
    fn from_cycle_error(tcx: TyCtxt<'tcx>) -> Self {
        // SAFETY: This is never called when `Self` is not `Ty<'tcx>`.
        // FIXME: Represent the above fact in the trait system somehow.
        unsafe { std::mem::transmute::<Ty<'tcx>, Ty<'_>>(tcx.types.err) }
    }
}

impl<'tcx> Value<'tcx> for ty::SymbolName {
    fn from_cycle_error(_: TyCtxt<'tcx>) -> Self {
        ty::SymbolName { name: Symbol::intern("<error>") }
    }
}

impl<'tcx> Value<'tcx> for AdtSizedConstraint<'_> {
    fn from_cycle_error(tcx: TyCtxt<'tcx>) -> Self {
        // SAFETY: This is never called when `Self` is not `AdtSizedConstraint<'tcx>`.
        // FIXME: Represent the above fact in the trait system somehow.
        unsafe {
            std::mem::transmute::<AdtSizedConstraint<'tcx>, AdtSizedConstraint<'_>>(
                AdtSizedConstraint(tcx.intern_type_list(&[tcx.types.err])),
            )
        }
    }
}
