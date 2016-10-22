// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hair::cx::Cx;
use rustc::middle::region::{CodeExtent, CodeExtentData, ROOT_CODE_EXTENT};
use rustc::ty::{self, Ty};
use rustc::mir::repr::*;
use rustc::util::nodemap::NodeMap;
use rustc::hir;
use syntax::abi::Abi;
use syntax::ast;
use syntax::parse::token::keywords;
use syntax_pos::Span;

use rustc_data_structures::indexed_vec::{IndexVec, Idx};

use std::u32;

pub struct Builder<'a, 'gcx: 'a+'tcx, 'tcx: 'a> {
    hir: Cx<'a, 'gcx, 'tcx>,
    cfg: CFG<'tcx>,

    fn_span: Span,
    arg_count: usize,

    /// the current set of scopes, updated as we traverse;
    /// see the `scope` module for more details
    scopes: Vec<scope::Scope<'tcx>>,

    ///  for each scope, a span of blocks that defines it;
    ///  we track these for use in region and borrow checking,
    ///  but these are liable to get out of date once optimization
    ///  begins. They are also hopefully temporary, and will be
    ///  no longer needed when we adopt graph-based regions.
    scope_auxiliary: IndexVec<ScopeId, ScopeAuxiliary>,

    /// the current set of loops; see the `scope` module for more
    /// details
    loop_scopes: Vec<scope::LoopScope>,

    /// the vector of all scopes that we have created thus far;
    /// we track this for debuginfo later
    visibility_scopes: IndexVec<VisibilityScope, VisibilityScopeData>,
    visibility_scope: VisibilityScope,

    /// Maps node ids of variable bindings to the `Local`s created for them.
    var_indices: NodeMap<Local>,
    local_decls: IndexVec<Local, LocalDecl<'tcx>>,
    unit_temp: Option<Lvalue<'tcx>>,

    /// cached block with the RESUME terminator; this is created
    /// when first set of cleanups are built.
    cached_resume_block: Option<BasicBlock>,
    /// cached block with the RETURN terminator
    cached_return_block: Option<BasicBlock>,
}

struct CFG<'tcx> {
    basic_blocks: IndexVec<BasicBlock, BasicBlockData<'tcx>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScopeId(u32);

impl Idx for ScopeId {
    fn new(index: usize) -> ScopeId {
        assert!(index < (u32::MAX as usize));
        ScopeId(index as u32)
    }

    fn index(self) -> usize {
        self.0 as usize
    }
}

/// For each scope, we track the extent (from the HIR) and a
/// single-entry-multiple-exit subgraph that contains all the
/// statements/terminators within it.
///
/// This information is separated out from the main `ScopeData`
/// because it is short-lived. First, the extent contains node-ids,
/// so it cannot be saved and re-loaded. Second, any optimization will mess up
/// the dominator/postdominator information.
///
/// The intention is basically to use this information to do
/// regionck/borrowck and then throw it away once we are done.
pub struct ScopeAuxiliary {
    /// extent of this scope from the MIR.
    pub extent: CodeExtent,

    /// "entry point": dominator of all nodes in the scope
    pub dom: Location,

    /// "exit points": mutual postdominators of all nodes in the scope
    pub postdoms: Vec<Location>,
}

pub type ScopeAuxiliaryVec = IndexVec<ScopeId, ScopeAuxiliary>;

///////////////////////////////////////////////////////////////////////////
/// The `BlockAnd` "monad" packages up the new basic block along with a
/// produced value (sometimes just unit, of course). The `unpack!`
/// macro (and methods below) makes working with `BlockAnd` much more
/// convenient.

#[must_use] // if you don't use one of these results, you're leaving a dangling edge
pub struct BlockAnd<T>(BasicBlock, T);

trait BlockAndExtension {
    fn and<T>(self, v: T) -> BlockAnd<T>;
    fn unit(self) -> BlockAnd<()>;
}

impl BlockAndExtension for BasicBlock {
    fn and<T>(self, v: T) -> BlockAnd<T> {
        BlockAnd(self, v)
    }

    fn unit(self) -> BlockAnd<()> {
        BlockAnd(self, ())
    }
}

/// Update a block pointer and return the value.
/// Use it like `let x = unpack!(block = self.foo(block, foo))`.
macro_rules! unpack {
    ($x:ident = $c:expr) => {
        {
            let BlockAnd(b, v) = $c;
            $x = b;
            v
        }
    };

    ($c:expr) => {
        {
            let BlockAnd(b, ()) = $c;
            b
        }
    };
}

///////////////////////////////////////////////////////////////////////////
/// the main entry point for building MIR for a function

pub fn construct_fn<'a, 'gcx, 'tcx, A>(hir: Cx<'a, 'gcx, 'tcx>,
                                       fn_id: ast::NodeId,
                                       arguments: A,
                                       return_ty: Ty<'gcx>,
                                       ast_block: &'gcx hir::Block)
                                       -> (Mir<'tcx>, ScopeAuxiliaryVec)
    where A: Iterator<Item=(Ty<'gcx>, Option<&'gcx hir::Pat>)>
{
    let arguments: Vec<_> = arguments.collect();

    let tcx = hir.tcx();
    let span = tcx.map.span(fn_id);
    let mut builder = Builder::new(hir, span, arguments.len(), return_ty);

    let body_id = ast_block.id;
    let call_site_extent =
        tcx.region_maps.lookup_code_extent(
            CodeExtentData::CallSiteScope { fn_id: fn_id, body_id: body_id });
    let arg_extent =
        tcx.region_maps.lookup_code_extent(
            CodeExtentData::ParameterScope { fn_id: fn_id, body_id: body_id });
    let mut block = START_BLOCK;
    unpack!(block = builder.in_scope(call_site_extent, block, |builder| {
        unpack!(block = builder.in_scope(arg_extent, block, |builder| {
            builder.args_and_body(block, return_ty, &arguments, arg_extent, ast_block)
        }));
        // Attribute epilogue to function's closing brace
        let fn_end = Span { lo: span.hi, ..span };
        let source_info = builder.source_info(fn_end);
        let return_block = builder.return_block();
        builder.cfg.terminate(block, source_info,
                              TerminatorKind::Goto { target: return_block });
        builder.cfg.terminate(return_block, source_info,
                              TerminatorKind::Return);
        return_block.unit()
    }));
    assert_eq!(block, builder.return_block());

    let mut spread_arg = None;
    match tcx.node_id_to_type(fn_id).sty {
        ty::TyFnDef(_, _, f) if f.abi == Abi::RustCall => {
            // RustCall pseudo-ABI untuples the last argument.
            spread_arg = Some(Local::new(arguments.len()));
        }
        _ => {}
    }

    // Gather the upvars of a closure, if any.
    let upvar_decls: Vec<_> = tcx.with_freevars(fn_id, |freevars| {
        freevars.iter().map(|fv| {
            let var_id = tcx.map.as_local_node_id(fv.def.def_id()).unwrap();
            let by_ref = tcx.upvar_capture(ty::UpvarId {
                var_id: var_id,
                closure_expr_id: fn_id
            }).map_or(false, |capture| match capture {
                ty::UpvarCapture::ByValue => false,
                ty::UpvarCapture::ByRef(..) => true
            });
            let mut decl = UpvarDecl {
                debug_name: keywords::Invalid.name(),
                by_ref: by_ref
            };
            if let Some(hir::map::NodeLocal(pat)) = tcx.map.find(var_id) {
                if let hir::PatKind::Binding(_, ref ident, _) = pat.node {
                    decl.debug_name = ident.node;
                }
            }
            decl
        }).collect()
    });

    let (mut mir, aux) = builder.finish(upvar_decls, return_ty);
    mir.spread_arg = spread_arg;
    (mir, aux)
}

pub fn construct_const<'a, 'gcx, 'tcx>(hir: Cx<'a, 'gcx, 'tcx>,
                                       item_id: ast::NodeId,
                                       ast_expr: &'tcx hir::Expr)
                                       -> (Mir<'tcx>, ScopeAuxiliaryVec) {
    let tcx = hir.tcx();
    let ty = tcx.expr_ty_adjusted(ast_expr);
    let span = tcx.map.span(item_id);
    let mut builder = Builder::new(hir, span, 0, ty);

    let extent = tcx.region_maps.temporary_scope(ast_expr.id)
                    .unwrap_or(ROOT_CODE_EXTENT);
    let mut block = START_BLOCK;
    let _ = builder.in_scope(extent, block, |builder| {
        let expr = builder.hir.mirror(ast_expr);
        unpack!(block = builder.into(&Lvalue::Local(RETURN_POINTER), block, expr));

        let source_info = builder.source_info(span);
        let return_block = builder.return_block();
        builder.cfg.terminate(block, source_info,
                              TerminatorKind::Goto { target: return_block });
        builder.cfg.terminate(return_block, source_info,
                              TerminatorKind::Return);

        return_block.unit()
    });

    builder.finish(vec![], ty)
}

impl<'a, 'gcx, 'tcx> Builder<'a, 'gcx, 'tcx> {
    fn new(hir: Cx<'a, 'gcx, 'tcx>,
           span: Span,
           arg_count: usize,
           return_ty: Ty<'tcx>)
           -> Builder<'a, 'gcx, 'tcx> {
        let mut builder = Builder {
            hir: hir,
            cfg: CFG { basic_blocks: IndexVec::new() },
            fn_span: span,
            arg_count: arg_count,
            scopes: vec![],
            visibility_scopes: IndexVec::new(),
            visibility_scope: ARGUMENT_VISIBILITY_SCOPE,
            scope_auxiliary: IndexVec::new(),
            loop_scopes: vec![],
            local_decls: IndexVec::from_elem_n(LocalDecl::new_return_pointer(return_ty), 1),
            var_indices: NodeMap(),
            unit_temp: None,
            cached_resume_block: None,
            cached_return_block: None
        };

        assert_eq!(builder.cfg.start_new_block(), START_BLOCK);
        assert_eq!(builder.new_visibility_scope(span), ARGUMENT_VISIBILITY_SCOPE);
        builder.visibility_scopes[ARGUMENT_VISIBILITY_SCOPE].parent_scope = None;

        builder
    }

    fn finish(self,
              upvar_decls: Vec<UpvarDecl>,
              return_ty: Ty<'tcx>)
              -> (Mir<'tcx>, ScopeAuxiliaryVec) {
        for (index, block) in self.cfg.basic_blocks.iter().enumerate() {
            if block.terminator.is_none() {
                span_bug!(self.fn_span, "no terminator on block {:?}", index);
            }
        }

        (Mir::new(self.cfg.basic_blocks,
                  self.visibility_scopes,
                  IndexVec::new(),
                  return_ty,
                  self.local_decls,
                  self.arg_count,
                  upvar_decls,
                  self.fn_span
        ), self.scope_auxiliary)
    }

    fn args_and_body(&mut self,
                     mut block: BasicBlock,
                     return_ty: Ty<'tcx>,
                     arguments: &[(Ty<'gcx>, Option<&'gcx hir::Pat>)],
                     argument_extent: CodeExtent,
                     ast_block: &'gcx hir::Block)
                     -> BlockAnd<()>
    {
        // Allocate locals for the function arguments
        for &(ty, pattern) in arguments.iter() {
            // If this is a simple binding pattern, give the local a nice name for debuginfo.
            let mut name = None;
            if let Some(pat) = pattern {
                if let hir::PatKind::Binding(_, ref ident, _) = pat.node {
                    name = Some(ident.node);
                }
            }

            self.local_decls.push(LocalDecl {
                mutability: Mutability::Not,
                ty: ty,
                source_info: None,
                name: name,
            });
        }

        let mut scope = None;
        // Bind the argument patterns
        for (index, &(ty, pattern)) in arguments.iter().enumerate() {
            // Function arguments always get the first Local indices after the return pointer
            let lvalue = Lvalue::Local(Local::new(index + 1));

            if let Some(pattern) = pattern {
                let pattern = self.hir.irrefutable_pat(pattern);
                scope = self.declare_bindings(scope, ast_block.span, &pattern);
                unpack!(block = self.lvalue_into_pattern(block, pattern, &lvalue));
            }

            // Make sure we drop (parts of) the argument even when not matched on.
            self.schedule_drop(pattern.as_ref().map_or(ast_block.span, |pat| pat.span),
                               argument_extent, &lvalue, ty);

        }

        // Enter the argument pattern bindings visibility scope, if it exists.
        if let Some(visibility_scope) = scope {
            self.visibility_scope = visibility_scope;
        }

        // FIXME(#32959): temporary hack for the issue at hand
        let return_is_unit = return_ty.is_nil();
        // start the first basic block and translate the body
        unpack!(block = self.ast_block(&Lvalue::Local(RETURN_POINTER),
                return_is_unit, block, ast_block));

        block.unit()
    }

    fn get_unit_temp(&mut self) -> Lvalue<'tcx> {
        match self.unit_temp {
            Some(ref tmp) => tmp.clone(),
            None => {
                let ty = self.hir.unit_ty();
                let tmp = self.temp(ty);
                self.unit_temp = Some(tmp.clone());
                tmp
            }
        }
    }

    fn return_block(&mut self) -> BasicBlock {
        match self.cached_return_block {
            Some(rb) => rb,
            None => {
                let rb = self.cfg.start_new_block();
                self.cached_return_block = Some(rb);
                rb
            }
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// Builder methods are broken up into modules, depending on what kind
// of thing is being translated. Note that they use the `unpack` macro
// above extensively.

mod block;
mod cfg;
mod expr;
mod into;
mod matches;
mod misc;
mod scope;
