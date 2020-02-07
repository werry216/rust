use crate::prelude::*;

use rustc::mir::StatementKind::*;
use rustc_index::vec::IndexVec;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SsaKind {
    NotSsa,
    Ssa,
}

pub fn analyze(fx: &FunctionCx<'_, '_, impl Backend>) -> IndexVec<Local, SsaKind> {
    let mut flag_map = fx.mir.local_decls.iter().map(|local_decl| {
        if fx.clif_type(fx.monomorphize(&local_decl.ty)).is_some() {
            SsaKind::Ssa
        } else {
            SsaKind::NotSsa
        }
    }).collect::<IndexVec<Local, SsaKind>>();

    for bb in fx.mir.basic_blocks().iter() {
        for stmt in bb.statements.iter() {
            match &stmt.kind {
                Assign(place_and_rval) => match &place_and_rval.1 {
                    Rvalue::Ref(_, _, place) => {
                        not_ssa(&mut flag_map, place.local)
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        match &bb.terminator().kind {
            TerminatorKind::Call { destination, .. } => {
                if let Some((dest_place, _dest_bb)) = destination {
                    let dest_layout = fx.layout_of(fx.monomorphize(&dest_place.ty(&fx.mir.local_decls, fx.tcx).ty));
                    if !crate::abi::can_return_to_ssa_var(fx.tcx, dest_layout) {
                        not_ssa(&mut flag_map, dest_place.local)
                    }
                }
            }
            _ => {}
        }
    }

    flag_map
}

fn not_ssa(flag_map: &mut IndexVec<Local, SsaKind>, local: Local) {
    flag_map[local] = SsaKind::NotSsa;
}
