use crate::{
    transform::{MirPass, MirSource},
    util::patch::MirPatch,
};
use rustc_middle::mir::*;
use rustc_middle::ty::{Ty, TyCtxt};
use std::{borrow::Cow, fmt::Debug};

/// This pass optimizes something like
/// ```text
/// let x: Option<()>;
/// let y: Option<()>;
/// match (x,y) {
///     (Some(_), Some(_)) => {0},
///     _ => {1}
/// }
/// ```
/// into something like
/// ```text
/// let x: Option<()>;
/// let y: Option<()>;
/// let discriminant_x = // get discriminant of x
/// let discriminant_y = // get discriminant of y
/// if discriminant_x != discriminant_y || discriminant_x == None {1} else {0}
/// ```
pub struct EarlyOtherwiseBranch;

impl<'tcx> MirPass<'tcx> for EarlyOtherwiseBranch {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, source: MirSource<'tcx>, body: &mut Body<'tcx>) {
        if tcx.sess.opts.debugging_opts.mir_opt_level < 3 {
            return;
        }
        trace!("running EarlyOtherwiseBranch on {:?}", source);
        // we are only interested in this bb if the terminator is a switchInt
        let bbs_with_switch =
            body.basic_blocks().iter_enumerated().filter(|(_, bb)| is_switch(bb.terminator()));

        let opts_to_apply: Vec<OptimizationToApply<'tcx>> = bbs_with_switch
            .flat_map(|(bb_idx, bb)| {
                let switch = bb.terminator();
                let helper = Helper { body, tcx };
                let infos = helper.go(bb, switch)?;
                Some(OptimizationToApply { infos, basic_block_first_switch: bb_idx })
            })
            .collect();

        for opt_to_apply in opts_to_apply {
            trace!("SUCCESS: found optimization possibility to apply: {:?}", &opt_to_apply);
            // create the patch using MirPatch
            let mut patch = MirPatch::new(body);

            // create temp to store second discriminant in
            let discr_type = opt_to_apply.infos[0].second_switch_info.discr_ty;
            let discr_span = opt_to_apply.infos[0].second_switch_info.discr_source_info.span;
            let temp = patch.new_temp(discr_type, discr_span);
            let statements_before =
                body.basic_blocks()[opt_to_apply.basic_block_first_switch].statements.len();
            let end_of_block_location = Location {
                block: opt_to_apply.basic_block_first_switch,
                statement_index: statements_before,
            };
            patch.add_statement(end_of_block_location, StatementKind::StorageLive(temp));

            // create assignment of discriminant
            let place_of_adt_to_get_discriminant_of =
                opt_to_apply.infos[0].second_switch_info.place_of_adt_discr_read;
            patch.add_assign(
                end_of_block_location,
                Place::from(temp),
                Rvalue::Discriminant(place_of_adt_to_get_discriminant_of),
            );

            // create temp to store NotEqual comparison between the two discriminants
            let not_equal = BinOp::Ne;
            let not_equal_res_type = not_equal.ty(tcx, discr_type, discr_type);
            let not_equal_temp = patch.new_temp(not_equal_res_type, discr_span);
            patch.add_statement(end_of_block_location, StatementKind::StorageLive(not_equal_temp));

            // create NotEqual comparison between the two discriminants
            let first_descriminant_place =
                opt_to_apply.infos[0].first_switch_info.discr_used_in_switch;
            let not_equal_rvalue = Rvalue::BinaryOp(
                not_equal,
                Operand::Copy(Place::from(temp)),
                Operand::Copy(Place::from(first_descriminant_place)),
            );
            patch.add_statement(
                end_of_block_location,
                StatementKind::Assign(box (Place::from(not_equal_temp), not_equal_rvalue)),
            );

            let (mut targets_to_jump_to, values_to_jump_to): (Vec<_>, Vec<_>) = opt_to_apply
                .infos
                .iter()
                .flat_map(|x| x.second_switch_info.targets_with_values.iter())
                .cloned()
                .unzip();

            // add otherwise case in the end
            targets_to_jump_to.push(opt_to_apply.infos[0].first_switch_info.otherwise_bb);
            // new block that jumps to the correct discriminant case. This block is switched to if the discriminants are equal
            let new_switch_data = BasicBlockData::new(Some(Terminator {
                source_info: opt_to_apply.infos[0].second_switch_info.discr_source_info,
                kind: TerminatorKind::SwitchInt {
                    // the first and second discriminants are equal, so just pick one
                    discr: Operand::Copy(first_descriminant_place),
                    switch_ty: discr_type,
                    values: Cow::from(values_to_jump_to),
                    targets: targets_to_jump_to,
                },
            }));

            let new_switch_bb = patch.new_block(new_switch_data);

            // switch on the NotEqual. If true, then jump to the `otherwise` case.
            // If false, then jump to a basic block that then jumps to the correct disciminant case
            let true_case = opt_to_apply.infos[0].first_switch_info.otherwise_bb;
            let false_case = new_switch_bb;
            patch.patch_terminator(
                opt_to_apply.basic_block_first_switch,
                TerminatorKind::if_(
                    tcx,
                    Operand::Move(Place::from(not_equal_temp)),
                    true_case,
                    false_case,
                ),
            );

            // generate StorageDead for the temp not in use anymore. We use the not_equal_temp in the switch, so we can't mark that dead
            patch.add_statement(end_of_block_location, StatementKind::StorageDead(temp));

            patch.apply(body);
        }
    }
}

fn is_switch<'tcx>(terminator: &Terminator<'tcx>) -> bool {
    match terminator.kind {
        TerminatorKind::SwitchInt { .. } => true,
        _ => false,
    }
}

struct Helper<'a, 'tcx> {
    body: &'a Body<'tcx>,
    tcx: TyCtxt<'tcx>,
}

#[derive(Debug, Clone)]
struct SwitchDiscriminantInfo<'tcx> {
    /// Type of the discriminant being switched on
    discr_ty: Ty<'tcx>,
    /// The basic block that the otherwise branch points to
    otherwise_bb: BasicBlock,
    /// Target along with the value being branched from. Otherwise is not included
    targets_with_values: Vec<(BasicBlock, u128)>,
    discr_source_info: SourceInfo,
    /// The place of the discriminant used in the switch
    discr_used_in_switch: Place<'tcx>,
    /// The place of the adt that has its discriminant read
    place_of_adt_discr_read: Place<'tcx>,
    /// The type of the adt that has its discriminant read
    type_adt_matched_on: Ty<'tcx>,
}

#[derive(Debug)]
struct OptimizationToApply<'tcx> {
    infos: Vec<OptimizationInfo<'tcx>>,
    /// Basic block of the original first switch
    basic_block_first_switch: BasicBlock,
}

#[derive(Debug)]
struct OptimizationInfo<'tcx> {
    /// Info about the first switch and discriminant
    first_switch_info: SwitchDiscriminantInfo<'tcx>,
    /// Info about the second switch and discriminant
    second_switch_info: SwitchDiscriminantInfo<'tcx>,
}

impl<'a, 'tcx> Helper<'a, 'tcx> {
    pub fn go(
        &self,
        bb: &BasicBlockData<'tcx>,
        switch: &Terminator<'tcx>,
    ) -> Option<Vec<OptimizationInfo<'tcx>>> {
        // try to find the statement that defines the discriminant that is used for the switch
        let discr = self.find_switch_discriminant_info(bb, switch)?;

        // go through each target, finding a discriminant read, and a switch
        let results = discr.targets_with_values.iter().map(|(target, value)| {
            self.find_discriminant_switch_pairing(&discr, target.clone(), value.clone())
        });

        // if the optimization did not apply for one of the targets, then abort
        if results.clone().any(|x| x.is_none()) || results.len() == 0 {
            trace!("NO: not all of the targets matched the pattern for optimization");
            return None;
        }

        Some(results.flatten().collect())
    }

    fn find_discriminant_switch_pairing(
        &self,
        discr_info: &SwitchDiscriminantInfo<'tcx>,
        target: BasicBlock,
        value: u128,
    ) -> Option<OptimizationInfo<'tcx>> {
        let bb = &self.body.basic_blocks()[target];
        // find switch
        let terminator = bb.terminator();
        if is_switch(terminator) {
            let this_bb_discr_info = self.find_switch_discriminant_info(bb, terminator)?;

            // the types of the two adts matched on have to be equalfor this optimization to apply
            if discr_info.type_adt_matched_on != this_bb_discr_info.type_adt_matched_on {
                trace!(
                    "NO: types do not match. LHS: {:?}, RHS: {:?}",
                    discr_info.type_adt_matched_on,
                    this_bb_discr_info.type_adt_matched_on
                );
                return None;
            }

            // the otherwise branch of the two switches have to point to the same bb
            if discr_info.otherwise_bb != this_bb_discr_info.otherwise_bb {
                trace!("NO: otherwise target is not the same");
                return None;
            }

            // check that the value being matched on is the same. The
            if this_bb_discr_info.targets_with_values.iter().find(|x| x.1 == value).is_none() {
                trace!("NO: values being matched on are not the same");
                return None;
            }

            // only allow optimization if the left and right of the tuple being matched are the same variants.
            // so the following should not optimize
            //  ```rust
            // let x: Option<()>;
            // let y: Option<()>;
            // match (x,y) {
            //     (Some(_), None) => {},
            //     _ => {}
            // }
            //  ```
            // We check this by seeing that the value of the first discriminant is the only other discriminant value being used as a target in the second switch
            if !(this_bb_discr_info.targets_with_values.len() == 1
                && this_bb_discr_info.targets_with_values[0].1 == value)
            {
                trace!(
                    "NO: The second switch did not have only 1 target (besides otherwise) that had the same value as the value from the first switch that got us here"
                );
                return None;
            }

            // if we reach this point, the optimization applies, and we should be able to optimize this case
            // store the info that is needed to apply the optimization

            Some(OptimizationInfo {
                first_switch_info: discr_info.clone(),
                second_switch_info: this_bb_discr_info,
            })
        } else {
            None
        }
    }

    fn find_switch_discriminant_info(
        &self,
        bb: &BasicBlockData<'tcx>,
        switch: &Terminator<'tcx>,
    ) -> Option<SwitchDiscriminantInfo<'tcx>> {
        match &switch.kind {
            TerminatorKind::SwitchInt { discr, targets, values, .. } => {
                let discr_local = discr.place()?.as_local()?;
                // the declaration of the discriminant read. Place of this read is being used in the switch
                let discr_decl = &self.body.local_decls()[discr_local];
                let discr_ty = discr_decl.ty;
                // the otherwise target lies as the last element
                let otherwise_bb = targets.get(values.len())?.clone();
                let targets_with_values = targets
                    .iter()
                    .zip(values.iter())
                    .map(|(t, v)| (t.clone(), v.clone()))
                    .collect();

                // find the place of the adt where the discriminant is being read from
                // assume this is the last statement of the block
                let place_of_adt_discr_read = match bb.statements.last()?.kind {
                    StatementKind::Assign(box (_, Rvalue::Discriminant(adt_place))) => {
                        Some(adt_place)
                    }
                    _ => None,
                }?;

                let type_adt_matched_on = place_of_adt_discr_read.ty(self.body, self.tcx).ty;

                Some(SwitchDiscriminantInfo {
                    discr_used_in_switch: discr.place()?,
                    discr_ty,
                    otherwise_bb,
                    targets_with_values,
                    discr_source_info: discr_decl.source_info,
                    place_of_adt_discr_read,
                    type_adt_matched_on,
                })
            }
            _ => unreachable!("must only be passed terminator that is a switch"),
        }
    }
}
