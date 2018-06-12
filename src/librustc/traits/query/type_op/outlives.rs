// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use infer::{InferCtxt, InferOk};
use traits::query::dropck_outlives::trivial_dropck_outlives;
use traits::query::Fallible;
use traits::ObligationCause;
use ty::subst::Kind;
use ty::{ParamEnv, Ty, TyCtxt};

#[derive(Debug)]
pub struct DropckOutlives<'tcx> {
    param_env: ParamEnv<'tcx>,
    dropped_ty: Ty<'tcx>,
}

impl<'tcx> DropckOutlives<'tcx> {
    pub fn new(param_env: ParamEnv<'tcx>, dropped_ty: Ty<'tcx>) -> Self {
        DropckOutlives {
            param_env,
            dropped_ty,
        }
    }
}

impl<'gcx, 'tcx> super::TypeOp<'gcx, 'tcx> for DropckOutlives<'tcx> {
    type Output = Vec<Kind<'tcx>>;

    fn trivial_noop(self, tcx: TyCtxt<'_, 'gcx, 'tcx>) -> Result<Self::Output, Self> {
        if trivial_dropck_outlives(tcx, self.dropped_ty) {
            Ok(vec![])
        } else {
            Err(self)
        }
    }

    fn perform(
        self,
        infcx: &InferCtxt<'_, 'gcx, 'tcx>,
    ) -> Fallible<InferOk<'tcx, Vec<Kind<'tcx>>>> {
        Ok(infcx
            .at(&ObligationCause::dummy(), self.param_env)
            .dropck_outlives(self.dropped_ty))
    }
}
