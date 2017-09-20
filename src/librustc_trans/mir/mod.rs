// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libc::c_uint;
use llvm::{self, ValueRef, BasicBlockRef};
use llvm::debuginfo::DIScope;
use rustc::ty::{self, TypeFoldable};
use rustc::ty::layout::{LayoutOf, FullLayout};
use rustc::mir::{self, Mir};
use rustc::ty::subst::Substs;
use rustc::infer::TransNormalize;
use rustc::session::config::FullDebugInfo;
use base;
use builder::Builder;
use common::{self, CrateContext, Funclet};
use debuginfo::{self, declare_local, VariableAccess, VariableKind, FunctionDebugContext};
use monomorphize::Instance;
use abi::{ArgAttribute, FnType};
use type_of::{self, LayoutLlvmExt};

use syntax_pos::{DUMMY_SP, NO_EXPANSION, BytePos, Span};
use syntax::symbol::keywords;

use std::iter;

use rustc_data_structures::bitvec::BitVector;
use rustc_data_structures::indexed_vec::{IndexVec, Idx};

pub use self::constant::trans_static_initializer;

use self::analyze::CleanupKind;
use self::lvalue::{Alignment, LvalueRef};
use rustc::mir::traversal;

use self::operand::{OperandRef, OperandValue};

/// Master context for translating MIR.
pub struct MirContext<'a, 'tcx:'a> {
    mir: &'a mir::Mir<'tcx>,

    debug_context: debuginfo::FunctionDebugContext,

    llfn: ValueRef,

    ccx: &'a CrateContext<'a, 'tcx>,

    fn_ty: FnType<'tcx>,

    /// When unwinding is initiated, we have to store this personality
    /// value somewhere so that we can load it and re-use it in the
    /// resume instruction. The personality is (afaik) some kind of
    /// value used for C++ unwinding, which must filter by type: we
    /// don't really care about it very much. Anyway, this value
    /// contains an alloca into which the personality is stored and
    /// then later loaded when generating the DIVERGE_BLOCK.
    personality_slot: Option<LvalueRef<'tcx>>,

    /// A `Block` for each MIR `BasicBlock`
    blocks: IndexVec<mir::BasicBlock, BasicBlockRef>,

    /// The funclet status of each basic block
    cleanup_kinds: IndexVec<mir::BasicBlock, analyze::CleanupKind>,

    /// When targeting MSVC, this stores the cleanup info for each funclet
    /// BB. This is initialized as we compute the funclets' head block in RPO.
    funclets: &'a IndexVec<mir::BasicBlock, Option<Funclet>>,

    /// This stores the landing-pad block for a given BB, computed lazily on GNU
    /// and eagerly on MSVC.
    landing_pads: IndexVec<mir::BasicBlock, Option<BasicBlockRef>>,

    /// Cached unreachable block
    unreachable_block: Option<BasicBlockRef>,

    /// The location where each MIR arg/var/tmp/ret is stored. This is
    /// usually an `LvalueRef` representing an alloca, but not always:
    /// sometimes we can skip the alloca and just store the value
    /// directly using an `OperandRef`, which makes for tighter LLVM
    /// IR. The conditions for using an `OperandRef` are as follows:
    ///
    /// - the type of the local must be judged "immediate" by `type_is_immediate`
    /// - the operand must never be referenced indirectly
    ///     - we should not take its address using the `&` operator
    ///     - nor should it appear in an lvalue path like `tmp.a`
    /// - the operand must be defined by an rvalue that can generate immediate
    ///   values
    ///
    /// Avoiding allocs can also be important for certain intrinsics,
    /// notably `expect`.
    locals: IndexVec<mir::Local, LocalRef<'tcx>>,

    /// Debug information for MIR scopes.
    scopes: IndexVec<mir::VisibilityScope, debuginfo::MirDebugScope>,

    /// If this function is being monomorphized, this contains the type substitutions used.
    param_substs: &'tcx Substs<'tcx>,
}

impl<'a, 'tcx> MirContext<'a, 'tcx> {
    pub fn monomorphize<T>(&self, value: &T) -> T
        where T: TransNormalize<'tcx>
    {
        self.ccx.tcx().trans_apply_param_substs(self.param_substs, value)
    }

    pub fn set_debug_loc(&mut self, bcx: &Builder, source_info: mir::SourceInfo) {
        let (scope, span) = self.debug_loc(source_info);
        debuginfo::set_source_location(&self.debug_context, bcx, scope, span);
    }

    pub fn debug_loc(&mut self, source_info: mir::SourceInfo) -> (DIScope, Span) {
        // Bail out if debug info emission is not enabled.
        match self.debug_context {
            FunctionDebugContext::DebugInfoDisabled |
            FunctionDebugContext::FunctionWithoutDebugInfo => {
                return (self.scopes[source_info.scope].scope_metadata, source_info.span);
            }
            FunctionDebugContext::RegularContext(_) =>{}
        }

        // In order to have a good line stepping behavior in debugger, we overwrite debug
        // locations of macro expansions with that of the outermost expansion site
        // (unless the crate is being compiled with `-Z debug-macros`).
        if source_info.span.ctxt() == NO_EXPANSION ||
           self.ccx.sess().opts.debugging_opts.debug_macros {
            let scope = self.scope_metadata_for_loc(source_info.scope, source_info.span.lo());
            (scope, source_info.span)
        } else {
            // Walk up the macro expansion chain until we reach a non-expanded span.
            // We also stop at the function body level because no line stepping can occur
            // at the level above that.
            let mut span = source_info.span;
            while span.ctxt() != NO_EXPANSION && span.ctxt() != self.mir.span.ctxt() {
                if let Some(info) = span.ctxt().outer().expn_info() {
                    span = info.call_site;
                } else {
                    break;
                }
            }
            let scope = self.scope_metadata_for_loc(source_info.scope, span.lo());
            // Use span of the outermost expansion site, while keeping the original lexical scope.
            (scope, span)
        }
    }

    // DILocations inherit source file name from the parent DIScope.  Due to macro expansions
    // it may so happen that the current span belongs to a different file than the DIScope
    // corresponding to span's containing visibility scope.  If so, we need to create a DIScope
    // "extension" into that file.
    fn scope_metadata_for_loc(&self, scope_id: mir::VisibilityScope, pos: BytePos)
                               -> llvm::debuginfo::DIScope {
        let scope_metadata = self.scopes[scope_id].scope_metadata;
        if pos < self.scopes[scope_id].file_start_pos ||
           pos >= self.scopes[scope_id].file_end_pos {
            let cm = self.ccx.sess().codemap();
            let defining_crate = self.debug_context.get_ref(DUMMY_SP).defining_crate;
            debuginfo::extend_scope_to_file(self.ccx,
                                            scope_metadata,
                                            &cm.lookup_char_pos(pos).file,
                                            defining_crate)
        } else {
            scope_metadata
        }
    }
}

enum LocalRef<'tcx> {
    Lvalue(LvalueRef<'tcx>),
    Operand(Option<OperandRef<'tcx>>),
}

impl<'a, 'tcx> LocalRef<'tcx> {
    fn new_operand(ccx: &CrateContext<'a, 'tcx>, layout: FullLayout<'tcx>) -> LocalRef<'tcx> {
        if layout.is_zst() {
            // Zero-size temporaries aren't always initialized, which
            // doesn't matter because they don't contain data, but
            // we need something in the operand.
            LocalRef::Operand(Some(OperandRef::new_zst(ccx, layout)))
        } else {
            LocalRef::Operand(None)
        }
    }
}

///////////////////////////////////////////////////////////////////////////

pub fn trans_mir<'a, 'tcx: 'a>(
    ccx: &'a CrateContext<'a, 'tcx>,
    llfn: ValueRef,
    mir: &'a Mir<'tcx>,
    instance: Instance<'tcx>,
    sig: ty::FnSig<'tcx>,
) {
    let fn_ty = FnType::new(ccx, sig, &[]);
    debug!("fn_ty: {:?}", fn_ty);
    let debug_context =
        debuginfo::create_function_debug_context(ccx, instance, sig, llfn, mir);
    let bcx = Builder::new_block(ccx, llfn, "start");

    if mir.basic_blocks().iter().any(|bb| bb.is_cleanup) {
        bcx.set_personality_fn(ccx.eh_personality());
    }

    let cleanup_kinds = analyze::cleanup_kinds(&mir);
    // Allocate a `Block` for every basic block, except
    // the start block, if nothing loops back to it.
    let reentrant_start_block = !mir.predecessors_for(mir::START_BLOCK).is_empty();
    let block_bcxs: IndexVec<mir::BasicBlock, BasicBlockRef> =
        mir.basic_blocks().indices().map(|bb| {
            if bb == mir::START_BLOCK && !reentrant_start_block {
                bcx.llbb()
            } else {
                bcx.build_sibling_block(&format!("{:?}", bb)).llbb()
            }
        }).collect();

    // Compute debuginfo scopes from MIR scopes.
    let scopes = debuginfo::create_mir_scopes(ccx, mir, &debug_context);
    let (landing_pads, funclets) = create_funclets(&bcx, &cleanup_kinds, &block_bcxs);

    let mut mircx = MirContext {
        mir,
        llfn,
        fn_ty,
        ccx,
        personality_slot: None,
        blocks: block_bcxs,
        unreachable_block: None,
        cleanup_kinds,
        landing_pads,
        funclets: &funclets,
        scopes,
        locals: IndexVec::new(),
        debug_context,
        param_substs: {
            assert!(!instance.substs.needs_infer());
            instance.substs
        },
    };

    let lvalue_locals = analyze::lvalue_locals(&mircx);

    // Allocate variable and temp allocas
    mircx.locals = {
        let args = arg_local_refs(&bcx, &mircx, &mircx.scopes, &lvalue_locals);

        let mut allocate_local = |local| {
            let decl = &mir.local_decls[local];
            let layout = bcx.ccx.layout_of(mircx.monomorphize(&decl.ty));
            assert!(!layout.ty.has_erasable_regions());

            if let Some(name) = decl.name {
                // User variable
                let debug_scope = mircx.scopes[decl.source_info.scope];
                let dbg = debug_scope.is_valid() && bcx.sess().opts.debuginfo == FullDebugInfo;

                if !lvalue_locals.contains(local.index()) && !dbg {
                    debug!("alloc: {:?} ({}) -> operand", local, name);
                    return LocalRef::new_operand(bcx.ccx, layout);
                }

                debug!("alloc: {:?} ({}) -> lvalue", local, name);
                let lvalue = LvalueRef::alloca(&bcx, layout, &name.as_str());
                if dbg {
                    let (scope, span) = mircx.debug_loc(decl.source_info);
                    declare_local(&bcx, &mircx.debug_context, name, layout.ty, scope,
                        VariableAccess::DirectVariable { alloca: lvalue.llval },
                        VariableKind::LocalVariable, span);
                }
                LocalRef::Lvalue(lvalue)
            } else {
                // Temporary or return pointer
                if local == mir::RETURN_POINTER && mircx.fn_ty.ret.is_indirect() {
                    debug!("alloc: {:?} (return pointer) -> lvalue", local);
                    let llretptr = llvm::get_param(llfn, 0);
                    LocalRef::Lvalue(LvalueRef::new_sized(llretptr,
                                                          layout,
                                                          Alignment::AbiAligned))
                } else if lvalue_locals.contains(local.index()) {
                    debug!("alloc: {:?} -> lvalue", local);
                    LocalRef::Lvalue(LvalueRef::alloca(&bcx, layout, &format!("{:?}", local)))
                } else {
                    // If this is an immediate local, we do not create an
                    // alloca in advance. Instead we wait until we see the
                    // definition and update the operand there.
                    debug!("alloc: {:?} -> operand", local);
                    LocalRef::new_operand(bcx.ccx, layout)
                }
            }
        };

        let retptr = allocate_local(mir::RETURN_POINTER);
        iter::once(retptr)
            .chain(args.into_iter())
            .chain(mir.vars_and_temps_iter().map(allocate_local))
            .collect()
    };

    // Branch to the START block, if it's not the entry block.
    if reentrant_start_block {
        bcx.br(mircx.blocks[mir::START_BLOCK]);
    }

    // Up until here, IR instructions for this function have explicitly not been annotated with
    // source code location, so we don't step into call setup code. From here on, source location
    // emitting should be enabled.
    debuginfo::start_emitting_source_locations(&mircx.debug_context);

    let rpo = traversal::reverse_postorder(&mir);
    let mut visited = BitVector::new(mir.basic_blocks().len());

    // Translate the body of each block using reverse postorder
    for (bb, _) in rpo {
        visited.insert(bb.index());
        mircx.trans_block(bb);
    }

    // Remove blocks that haven't been visited, or have no
    // predecessors.
    for bb in mir.basic_blocks().indices() {
        // Unreachable block
        if !visited.contains(bb.index()) {
            debug!("trans_mir: block {:?} was not visited", bb);
            unsafe {
                llvm::LLVMDeleteBasicBlock(mircx.blocks[bb]);
            }
        }
    }
}

fn create_funclets<'a, 'tcx>(
    bcx: &Builder<'a, 'tcx>,
    cleanup_kinds: &IndexVec<mir::BasicBlock, CleanupKind>,
    block_bcxs: &IndexVec<mir::BasicBlock, BasicBlockRef>)
    -> (IndexVec<mir::BasicBlock, Option<BasicBlockRef>>,
        IndexVec<mir::BasicBlock, Option<Funclet>>)
{
    block_bcxs.iter_enumerated().zip(cleanup_kinds).map(|((bb, &llbb), cleanup_kind)| {
        match *cleanup_kind {
            CleanupKind::Funclet if base::wants_msvc_seh(bcx.sess()) => {
                let cleanup_bcx = bcx.build_sibling_block(&format!("funclet_{:?}", bb));
                let cleanup = cleanup_bcx.cleanup_pad(None, &[]);
                cleanup_bcx.br(llbb);
                (Some(cleanup_bcx.llbb()), Some(Funclet::new(cleanup)))
            }
            _ => (None, None)
        }
    }).unzip()
}

/// Produce, for each argument, a `ValueRef` pointing at the
/// argument's value. As arguments are lvalues, these are always
/// indirect.
fn arg_local_refs<'a, 'tcx>(bcx: &Builder<'a, 'tcx>,
                            mircx: &MirContext<'a, 'tcx>,
                            scopes: &IndexVec<mir::VisibilityScope, debuginfo::MirDebugScope>,
                            lvalue_locals: &BitVector)
                            -> Vec<LocalRef<'tcx>> {
    let mir = mircx.mir;
    let tcx = bcx.tcx();
    let mut idx = 0;
    let mut llarg_idx = mircx.fn_ty.ret.is_indirect() as usize;

    // Get the argument scope, if it exists and if we need it.
    let arg_scope = scopes[mir::ARGUMENT_VISIBILITY_SCOPE];
    let arg_scope = if arg_scope.is_valid() && bcx.sess().opts.debuginfo == FullDebugInfo {
        Some(arg_scope.scope_metadata)
    } else {
        None
    };

    let deref_op = unsafe {
        [llvm::LLVMRustDIBuilderCreateOpDeref()]
    };

    mir.args_iter().enumerate().map(|(arg_index, local)| {
        let arg_decl = &mir.local_decls[local];

        let name = if let Some(name) = arg_decl.name {
            name.as_str().to_string()
        } else {
            format!("arg{}", arg_index)
        };

        if Some(local) == mir.spread_arg {
            // This argument (e.g. the last argument in the "rust-call" ABI)
            // is a tuple that was spread at the ABI level and now we have
            // to reconstruct it into a tuple local variable, from multiple
            // individual LLVM function arguments.

            let arg_ty = mircx.monomorphize(&arg_decl.ty);
            let tupled_arg_tys = match arg_ty.sty {
                ty::TyTuple(ref tys, _) => tys,
                _ => bug!("spread argument isn't a tuple?!")
            };

            let lvalue = LvalueRef::alloca(bcx, bcx.ccx.layout_of(arg_ty), &name);
            for i in 0..tupled_arg_tys.len() {
                let arg = &mircx.fn_ty.args[idx];
                idx += 1;
                arg.store_fn_arg(bcx, &mut llarg_idx, lvalue.project_field(bcx, i));
            }

            // Now that we have one alloca that contains the aggregate value,
            // we can create one debuginfo entry for the argument.
            arg_scope.map(|scope| {
                let variable_access = VariableAccess::DirectVariable {
                    alloca: lvalue.llval
                };
                declare_local(
                    bcx,
                    &mircx.debug_context,
                    arg_decl.name.unwrap_or(keywords::Invalid.name()),
                    arg_ty, scope,
                    variable_access,
                    VariableKind::ArgumentVariable(arg_index + 1),
                    DUMMY_SP
                );
            });

            return LocalRef::Lvalue(lvalue);
        }

        let arg = &mircx.fn_ty.args[idx];
        idx += 1;
        let lvalue = if arg.is_indirect() {
            // Don't copy an indirect argument to an alloca, the caller
            // already put it in a temporary alloca and gave it up
            // FIXME: lifetimes
            if arg.pad.is_some() {
                llarg_idx += 1;
            }
            let llarg = llvm::get_param(bcx.llfn(), llarg_idx as c_uint);
            bcx.set_value_name(llarg, &name);
            llarg_idx += 1;
            LvalueRef::new_sized(llarg, arg.layout, Alignment::AbiAligned)
        } else if !lvalue_locals.contains(local.index()) &&
                  !arg.nested.is_empty() {
            assert_eq!(arg.nested.len(), 2);
            let (a, b) = (&arg.nested[0], &arg.nested[1]);
            assert!(!a.is_ignore() && a.cast.is_none() && a.pad.is_none());
            assert!(!b.is_ignore() && b.cast.is_none() && b.pad.is_none());

            let mut a = llvm::get_param(bcx.llfn(), llarg_idx as c_uint);
            llarg_idx += 1;

            let mut b = llvm::get_param(bcx.llfn(), llarg_idx as c_uint);
            llarg_idx += 1;

            if common::type_is_fat_ptr(bcx.ccx, arg.layout.ty) {
                // FIXME(eddyb) As we can't perfectly represent the data and/or
                // vtable pointer in a fat pointers in Rust's typesystem, and
                // because we split fat pointers into two ArgType's, they're
                // not the right type so we have to cast them for now.
                let pointee = match arg.layout.ty.sty {
                    ty::TyRef(_, ty::TypeAndMut{ty, ..}) |
                    ty::TyRawPtr(ty::TypeAndMut{ty, ..}) => ty,
                    ty::TyAdt(def, _) if def.is_box() => arg.layout.ty.boxed_ty(),
                    _ => bug!()
                };
                let data_llty = bcx.ccx.layout_of(pointee).llvm_type(bcx.ccx);
                let meta_llty = type_of::unsized_info_ty(bcx.ccx, pointee);

                a = bcx.pointercast(a, data_llty.ptr_to());
                bcx.set_value_name(a, &(name.clone() + ".ptr"));
                b = bcx.pointercast(b, meta_llty);
                bcx.set_value_name(b, &(name + ".meta"));
            }

            return LocalRef::Operand(Some(OperandRef {
                val: OperandValue::Pair(a, b),
                layout: arg.layout
            }));
        } else if !lvalue_locals.contains(local.index()) &&
                  !arg.is_indirect() && arg.cast.is_none() &&
                  arg_scope.is_none() {
            if arg.is_ignore() {
                return LocalRef::new_operand(bcx.ccx, arg.layout);
            }

            // We don't have to cast or keep the argument in the alloca.
            // FIXME(eddyb): We should figure out how to use llvm.dbg.value instead
            // of putting everything in allocas just so we can use llvm.dbg.declare.
            if arg.pad.is_some() {
                llarg_idx += 1;
            }
            let llarg = llvm::get_param(bcx.llfn(), llarg_idx as c_uint);
            bcx.set_value_name(llarg, &name);
            llarg_idx += 1;
            let operand = OperandRef {
                val: OperandValue::Immediate(llarg),
                layout: arg.layout
            };
            return LocalRef::Operand(Some(operand.unpack_if_pair(bcx)));
        } else {
            let tmp = LvalueRef::alloca(bcx, arg.layout, &name);
            arg.store_fn_arg(bcx, &mut llarg_idx, tmp);
            tmp
        };
        arg_scope.map(|scope| {
            // Is this a regular argument?
            if arg_index > 0 || mir.upvar_decls.is_empty() {
                // The Rust ABI passes indirect variables using a pointer and a manual copy, so we
                // need to insert a deref here, but the C ABI uses a pointer and a copy using the
                // byval attribute, for which LLVM does the deref itself, so we must not add it.
                let variable_access = if arg.is_indirect() &&
                    !arg.attrs.contains(ArgAttribute::ByVal) {
                    VariableAccess::IndirectVariable {
                        alloca: lvalue.llval,
                        address_operations: &deref_op,
                    }
                } else {
                    VariableAccess::DirectVariable { alloca: lvalue.llval }
                };

                declare_local(
                    bcx,
                    &mircx.debug_context,
                    arg_decl.name.unwrap_or(keywords::Invalid.name()),
                    arg.layout.ty,
                    scope,
                    variable_access,
                    VariableKind::ArgumentVariable(arg_index + 1),
                    DUMMY_SP
                );
                return;
            }

            // Or is it the closure environment?
            let (closure_layout, env_ref) = match arg.layout.ty.sty {
                ty::TyRef(_, mt) | ty::TyRawPtr(mt) => (bcx.ccx.layout_of(mt.ty), true),
                _ => (arg.layout, false)
            };

            let upvar_tys = match closure_layout.ty.sty {
                ty::TyClosure(def_id, substs) |
                ty::TyGenerator(def_id, substs, _) => substs.upvar_tys(def_id, tcx),
                _ => bug!("upvar_decls with non-closure arg0 type `{}`", closure_layout.ty)
            };

            // Store the pointer to closure data in an alloca for debuginfo
            // because that's what the llvm.dbg.declare intrinsic expects.

            // FIXME(eddyb) this shouldn't be necessary but SROA seems to
            // mishandle DW_OP_plus not preceded by DW_OP_deref, i.e. it
            // doesn't actually strip the offset when splitting the closure
            // environment into its components so it ends up out of bounds.
            let env_ptr = if !env_ref {
                let alloc = LvalueRef::alloca(bcx,
                    bcx.ccx.layout_of(tcx.mk_mut_ptr(arg.layout.ty)),
                    "__debuginfo_env_ptr");
                bcx.store(lvalue.llval, alloc.llval, None);
                alloc.llval
            } else {
                lvalue.llval
            };

            for (i, (decl, ty)) in mir.upvar_decls.iter().zip(upvar_tys).enumerate() {
                let byte_offset_of_var_in_env = closure_layout.fields.offset(i).bytes();

                let ops = unsafe {
                    [llvm::LLVMRustDIBuilderCreateOpDeref(),
                     llvm::LLVMRustDIBuilderCreateOpPlus(),
                     byte_offset_of_var_in_env as i64,
                     llvm::LLVMRustDIBuilderCreateOpDeref()]
                };

                // The environment and the capture can each be indirect.

                // FIXME(eddyb) see above why we have to keep
                // a pointer in an alloca for debuginfo atm.
                let mut ops = if env_ref || true { &ops[..] } else { &ops[1..] };

                let ty = if let (true, &ty::TyRef(_, mt)) = (decl.by_ref, &ty.sty) {
                    mt.ty
                } else {
                    ops = &ops[..ops.len() - 1];
                    ty
                };

                let variable_access = VariableAccess::IndirectVariable {
                    alloca: env_ptr,
                    address_operations: &ops
                };
                declare_local(
                    bcx,
                    &mircx.debug_context,
                    decl.debug_name,
                    ty,
                    scope,
                    variable_access,
                    VariableKind::CapturedVariable,
                    DUMMY_SP
                );
            }
        });
        LocalRef::Lvalue(lvalue)
    }).collect()
}

mod analyze;
mod block;
mod constant;
pub mod lvalue;
pub mod operand;
mod rvalue;
mod statement;
