use std::collections::{BTreeMap, HashSet};
use std::ops::Not;

use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::entity::EntitySet;
use cranelift_codegen::ir::{InstructionData, Opcode, ProgramOrder, ValueDef};
use cranelift_codegen::ir::immediates::Offset32;

use crate::prelude::*;

/// Workaround for `StackSlot` not implementing `Ord`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct OrdStackSlot(StackSlot);

impl PartialOrd for OrdStackSlot {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        self.0.as_u32().partial_cmp(&rhs.0.as_u32())
    }
}

impl Ord for OrdStackSlot {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.0.as_u32().cmp(&rhs.0.as_u32())
    }
}

#[derive(Debug, Default)]
struct StackSlotUsage {
    stack_addr: HashSet<Inst>,
    stack_load: HashSet<Inst>,
    stack_store: HashSet<Inst>,
}

struct OptimizeContext<'a> {
    ctx: &'a mut Context,
    stack_slot_usage_map: BTreeMap<OrdStackSlot, StackSlotUsage>,
}

impl<'a> OptimizeContext<'a> {
    fn for_context(ctx: &'a mut Context) -> Self {
        ctx.flowgraph(); // Compute cfg and domtree.

        // Record all stack_addr, stack_load and stack_store instructions.
        let mut stack_slot_usage_map = BTreeMap::<OrdStackSlot, StackSlotUsage>::new();

        let mut cursor = FuncCursor::new(&mut ctx.func);
        while let Some(_ebb) = cursor.next_ebb() {
            while let Some(inst) = cursor.next_inst() {
                match cursor.func.dfg[inst] {
                    InstructionData::StackLoad {
                        opcode: Opcode::StackAddr,
                        stack_slot,
                        offset: _,
                    } => {
                        stack_slot_usage_map.entry(OrdStackSlot(stack_slot)).or_insert_with(StackSlotUsage::default).stack_addr.insert(inst);
                    }
                    InstructionData::StackLoad {
                        opcode: Opcode::StackLoad,
                        stack_slot,
                        offset: _,
                    } => {
                        stack_slot_usage_map.entry(OrdStackSlot(stack_slot)).or_insert_with(StackSlotUsage::default).stack_load.insert(inst);
                    }
                    InstructionData::StackStore {
                        opcode: Opcode::StackStore,
                        arg: _,
                        stack_slot,
                        offset: _,
                    } => {
                        stack_slot_usage_map.entry(OrdStackSlot(stack_slot)).or_insert_with(StackSlotUsage::default).stack_store.insert(inst);
                    }
                    _ => {}
                }
            }
        }

        OptimizeContext {
            ctx,
            stack_slot_usage_map,
        }
    }
}

pub(super) fn optimize_function(
    ctx: &mut Context,
    clif_comments: &mut crate::pretty_clif::CommentWriter,
    name: String, // FIXME remove
) {
    combine_stack_addr_with_load_store(&mut ctx.func);

    let mut opt_ctx = OptimizeContext::for_context(ctx);

    // FIXME Repeat following instructions until fixpoint.

    remove_unused_stack_addr_and_stack_load(&mut opt_ctx);

    println!("stack slot usage: {:?}", opt_ctx.stack_slot_usage_map);

    for (stack_slot, users) in opt_ctx.stack_slot_usage_map.iter_mut() {
        if users.stack_addr.is_empty().not() {
            // Stack addr leaked; there may be unknown loads and stores.
            // FIXME use stacked borrows to optimize
            continue;
        }

        for load in users.stack_load.clone().drain() {
            let load_ebb = opt_ctx.ctx.func.layout.inst_ebb(load).unwrap();
            let loaded_value = opt_ctx.ctx.func.dfg.inst_results(load)[0];
            let loaded_type = opt_ctx.ctx.func.dfg.value_type(loaded_value);

            let ctx = &*opt_ctx.ctx;
            let potential_stores = users.stack_store.iter().cloned().filter(|&store| {
                match spatial_overlap(&ctx.func, load, store) {
                    SpatialOverlap::No => false, // Can never be the source of the loaded value.
                    SpatialOverlap::Partial | SpatialOverlap::Full => true,
                }
            }).filter(|&store| {
                match temporal_order(ctx, load, store) {
                    TemporalOrder::NeverBefore => false, // Can never be the source of the loaded value.
                    TemporalOrder::MaybeBefore | TemporalOrder::DefinitivelyBefore => true,
                }
            }).collect::<Vec<Inst>>();

            for &store in &potential_stores {
                println!(
                    "Potential store -> load forwarding {} -> {} ({:?}, {:?})",
                    opt_ctx.ctx.func.dfg.display_inst(store, None),
                    opt_ctx.ctx.func.dfg.display_inst(load, None),
                    spatial_overlap(&opt_ctx.ctx.func, store, load),
                    temporal_order(&*opt_ctx.ctx, store, load),
                );
            }

            match *potential_stores {
                [] => println!("[{}] [BUG?] Reading uninitialized memory", name),
                [store] if spatial_overlap(&opt_ctx.ctx.func, store, load) == SpatialOverlap::Full && temporal_order(&opt_ctx.ctx, store, load) == TemporalOrder::DefinitivelyBefore => {
                    // Only one store could have been the origin of the value.
                    let store_ebb = opt_ctx.ctx.func.layout.inst_ebb(store).unwrap();
                    let stored_value = opt_ctx.ctx.func.dfg.inst_args(store)[0];
                    let stored_type = opt_ctx.ctx.func.dfg.value_type(stored_value);
                    if stored_type == loaded_type && store_ebb == load_ebb {
                        println!("Store to load forward {} -> {}", store, load);
                        opt_ctx.ctx.func.dfg.detach_results(load);
                        opt_ctx.ctx.func.dfg.replace(load).nop();
                        opt_ctx.ctx.func.dfg.change_to_alias(loaded_value, stored_value);
                        users.stack_load.remove(&load);
                    }
                }
                _ => {} // FIXME implement this
            }
        }

        for store in users.stack_store.clone().drain() {
            let ctx = &*opt_ctx.ctx;
            let potential_loads = users.stack_load.iter().cloned().filter(|&load| {
                match spatial_overlap(&ctx.func, store, load) {
                    SpatialOverlap::No => false, // Can never be the source of the loaded value.
                    SpatialOverlap::Partial | SpatialOverlap::Full => true,
                }
            }).filter(|&load| {
                match temporal_order(ctx, store, load) {
                    TemporalOrder::NeverBefore => false, // Can never be the source of the loaded value.
                    TemporalOrder::MaybeBefore | TemporalOrder::DefinitivelyBefore => true,
                }
            }).collect::<Vec<Inst>>();

            for &load in &potential_loads {
                println!(
                    "Potential load from store {} <- {} ({:?}, {:?})",
                    opt_ctx.ctx.func.dfg.display_inst(load, None),
                    opt_ctx.ctx.func.dfg.display_inst(store, None),
                    spatial_overlap(&ctx.func, store, load),
                    temporal_order(&*opt_ctx.ctx, store, load),
                );
            }

            if potential_loads.is_empty() {
                // Never loaded; can safely remove all stores and the stack slot.
                // FIXME also remove stores when there is always a next store before a load.
                println!("[{}] Remove dead stack store {} of {}", name, opt_ctx.ctx.func.dfg.display_inst(store, None), stack_slot.0);
                opt_ctx.ctx.func.dfg.replace(store).nop();
                users.stack_store.remove(&store);
            }
        }

        if users.stack_store.is_empty() && users.stack_load.is_empty() {
            // FIXME make stack_slot zero sized.
        }
    }

    println!();
}

fn combine_stack_addr_with_load_store(func: &mut Function) {
    // Turn load and store into stack_load and stack_store when possible.
    let mut cursor = FuncCursor::new(func);
    while let Some(_ebb) = cursor.next_ebb() {
        while let Some(inst) = cursor.next_inst() {
            match cursor.func.dfg[inst] {
                InstructionData::Load { opcode: Opcode::Load, arg: addr, flags: _, offset } => {
                    if cursor.func.dfg.ctrl_typevar(inst) == types::I128 || cursor.func.dfg.ctrl_typevar(inst).is_vector() {
                        continue; // WORKAROUD: stack_load.i128 not yet implemented
                    }
                    if let Some((stack_slot, stack_addr_offset)) = try_get_stack_slot_and_offset_for_addr(cursor.func, addr) {
                        if let Some(combined_offset) = offset.try_add_i64(stack_addr_offset.into()) {
                            let ty = cursor.func.dfg.ctrl_typevar(inst);
                            cursor.func.dfg.replace(inst).stack_load(ty, stack_slot, combined_offset);
                        }
                    }
                }
                InstructionData::Store { opcode: Opcode::Store, args: [value, addr], flags: _, offset } => {
                    if cursor.func.dfg.ctrl_typevar(inst) == types::I128 || cursor.func.dfg.ctrl_typevar(inst).is_vector() {
                        continue; // WORKAROUND: stack_store.i128 not yet implemented
                    }
                    if let Some((stack_slot, stack_addr_offset)) = try_get_stack_slot_and_offset_for_addr(cursor.func, addr) {
                        if let Some(combined_offset) = offset.try_add_i64(stack_addr_offset.into()) {
                            cursor.func.dfg.replace(inst).stack_store(value, stack_slot, combined_offset);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn remove_unused_stack_addr_and_stack_load(opt_ctx: &mut OptimizeContext) {
    // FIXME incrementally rebuild on each call?
    let mut stack_addr_load_insts_users = HashMap::<Inst, HashSet<Inst>>::new();

    let mut cursor = FuncCursor::new(&mut opt_ctx.ctx.func);
    while let Some(_ebb) = cursor.next_ebb() {
        while let Some(inst) = cursor.next_inst() {
            for &arg in cursor.func.dfg.inst_args(inst) {
                if let ValueDef::Result(arg_origin, 0) = cursor.func.dfg.value_def(arg) {
                    match cursor.func.dfg[arg_origin].opcode() {
                        Opcode::StackAddr | Opcode::StackLoad => {
                            stack_addr_load_insts_users.entry(arg_origin).or_insert_with(HashSet::new).insert(inst);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    for inst in stack_addr_load_insts_users.keys() {
        let mut is_recorded_stack_addr_or_stack_load = false;
        for stack_slot_users in opt_ctx.stack_slot_usage_map.values() {
            is_recorded_stack_addr_or_stack_load |= stack_slot_users.stack_addr.contains(inst) || stack_slot_users.stack_load.contains(inst);
        }
        assert!(is_recorded_stack_addr_or_stack_load);
    }

    // Replace all unused stack_addr and stack_load instructions with nop.
    for stack_slot_users in opt_ctx.stack_slot_usage_map.values_mut() {
        // FIXME remove clone
        for &inst in stack_slot_users.stack_addr.clone().iter() {
            if stack_addr_load_insts_users.get(&inst).map(|users| users.is_empty()).unwrap_or(true) {
                opt_ctx.ctx.func.dfg.detach_results(inst);
                opt_ctx.ctx.func.dfg.replace(inst).nop();
                stack_slot_users.stack_addr.remove(&inst);
            }
        }

        for &inst in stack_slot_users.stack_load.clone().iter() {
            if stack_addr_load_insts_users.get(&inst).map(|users| users.is_empty()).unwrap_or(true) {
                opt_ctx.ctx.func.dfg.detach_results(inst);
                opt_ctx.ctx.func.dfg.replace(inst).nop();
                stack_slot_users.stack_load.remove(&inst);
            }
        }
    }
}

fn try_get_stack_slot_and_offset_for_addr(func: &Function, addr: Value) -> Option<(StackSlot, Offset32)> {
    if let ValueDef::Result(addr_inst, 0) = func.dfg.value_def(addr) {
        if let InstructionData::StackLoad {
            opcode: Opcode::StackAddr,
            stack_slot,
            offset,
        } = func.dfg[addr_inst] {
            return Some((stack_slot, offset));
        }
    }
    None
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SpatialOverlap {
    No,
    Partial,
    Full,
}

fn spatial_overlap(func: &Function, src: Inst, dest: Inst) -> SpatialOverlap {
    fn inst_info(func: &Function, inst: Inst) -> (StackSlot, Offset32, u32) {
        match func.dfg[inst] {
            InstructionData::StackLoad {
                opcode: Opcode::StackAddr,
                stack_slot,
                offset,
            }
            | InstructionData::StackLoad {
                opcode: Opcode::StackLoad,
                stack_slot,
                offset,
            }
            | InstructionData::StackStore {
                opcode: Opcode::StackStore,
                stack_slot,
                offset,
                arg: _,
            } => (stack_slot, offset, func.dfg.ctrl_typevar(inst).bytes()),
            _ => unreachable!("{:?}", func.dfg[inst]),
        }
    }

    debug_assert_ne!(src, dest);

    let (src_ss, src_offset, src_size) = inst_info(func, src);
    let (dest_ss, dest_offset, dest_size) = inst_info(func, dest);

    if src_ss != dest_ss {
        return SpatialOverlap::No;
    }

    if src_offset == dest_offset && src_size == dest_size {
        return SpatialOverlap::Full;
    }

    let src_end: i64 = src_offset.try_add_i64(i64::from(src_size)).unwrap().into();
    let dest_end: i64 = dest_offset.try_add_i64(i64::from(dest_size)).unwrap().into();
    if src_end <= dest_offset.into() || dest_end <= src_offset.into() {
        return SpatialOverlap::No;
    }

    SpatialOverlap::Partial
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TemporalOrder {
    /// `src` will never be executed before `dest`.
    NeverBefore,

    /// `src` may be executed before `dest`.
    MaybeBefore,

    /// `src` will always be executed before `dest`.
    /// There may still be other instructions in between.
    DefinitivelyBefore,
}

fn temporal_order(ctx: &Context, src: Inst, dest: Inst) -> TemporalOrder {
    debug_assert_ne!(src, dest);

    let src_ebb = ctx.func.layout.inst_ebb(src).unwrap();
    let dest_ebb = ctx.func.layout.inst_ebb(dest).unwrap();
    if src_ebb == dest_ebb {
        use std::cmp::Ordering::*;
        match ctx.func.layout.cmp(src, dest) {
            Less => TemporalOrder::DefinitivelyBefore,
            Equal => unreachable!(),
            Greater => TemporalOrder::MaybeBefore, // FIXME use dominator to check for loops
        }
    } else {
        // FIXME O(stack_load count * ebb count)
        // FIXME reuse memory allocations
        // FIXME return DefinitivelyBefore is src dominates dest
        let mut visited = EntitySet::new();
        let mut todo = EntitySet::new();
        todo.insert(dest_ebb);
        while let Some(ebb) = todo.pop() {
            if visited.contains(ebb) {
                continue;
            }
            visited.insert(ebb);
            if ebb == src_ebb {
                return TemporalOrder::MaybeBefore;
            }
            for bb in ctx.cfg.pred_iter(ebb) {
                todo.insert(bb.ebb);
            }
        }
        TemporalOrder::NeverBefore
    }
}
