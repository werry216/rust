// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use build::{Location, ScopeAuxiliary};
use rustc::mir::repr::*;
use rustc::middle::ty::{self, TyCtxt};
use rustc_data_structures::fnv::FnvHashMap;
use std::io::{self, Write};
use syntax::ast::NodeId;
use syntax::codemap::Span;

const INDENT: &'static str = "    ";

/// Write out a human-readable textual representation for the given MIR.
pub fn write_mir_pretty<'a, 'tcx, I>(tcx: &TyCtxt<'tcx>,
                                     iter: I,
                                     w: &mut Write)
                                     -> io::Result<()>
    where I: Iterator<Item=(&'a NodeId, &'a Mir<'tcx>)>, 'tcx: 'a
{
    for (&node_id, mir) in iter {
        write_mir_fn(tcx, node_id, mir, w, None)?;
    }
    Ok(())
}

enum Annotation {
    EnterScope(ScopeId),
    ExitScope(ScopeId),
}

pub fn write_mir_fn<'tcx>(tcx: &TyCtxt<'tcx>,
                          node_id: NodeId,
                          mir: &Mir<'tcx>,
                          w: &mut Write,
                          auxiliary: Option<&Vec<ScopeAuxiliary>>)
                          -> io::Result<()> {
    // compute scope/entry exit annotations
    let mut annotations = FnvHashMap();
    if let Some(auxiliary) = auxiliary {
        for (index, auxiliary) in auxiliary.iter().enumerate() {
            let scope_id = ScopeId::new(index);

            annotations.entry(auxiliary.dom)
                       .or_insert(vec![])
                       .push(Annotation::EnterScope(scope_id));

            for &loc in &auxiliary.postdoms {
                annotations.entry(loc)
                           .or_insert(vec![])
                           .push(Annotation::ExitScope(scope_id));
            }
        }
    }

    write_mir_intro(tcx, node_id, mir, w)?;
    for block in mir.all_basic_blocks() {
        write_basic_block(tcx, block, mir, w, &annotations)?;
    }

    // construct a scope tree and write it out
    let mut scope_tree: FnvHashMap<Option<ScopeId>, Vec<ScopeId>> = FnvHashMap();
    for (index, scope_data) in mir.scopes.vec.iter().enumerate() {
        scope_tree.entry(scope_data.parent_scope)
                  .or_insert(vec![])
                  .push(ScopeId::new(index));
    }
    write_scope_tree(tcx, mir, auxiliary, &scope_tree, w, None, 1)?;

    writeln!(w, "}}")?;
    Ok(())
}

/// Write out a human-readable textual representation for the given basic block.
fn write_basic_block(tcx: &TyCtxt,
                     block: BasicBlock,
                     mir: &Mir,
                     w: &mut Write,
                     annotations: &FnvHashMap<Location, Vec<Annotation>>)
                     -> io::Result<()> {
    let data = mir.basic_block_data(block);

    // Basic block label at the top.
    writeln!(w, "\n{}{:?}: {{", INDENT, block)?;

    // List of statements in the middle.
    let mut current_location = Location { block: block, statement_index: 0 };
    for statement in &data.statements {
        if let Some(ref annotations) = annotations.get(&current_location) {
            for annotation in annotations.iter() {
                match *annotation {
                    Annotation::EnterScope(id) =>
                        writeln!(w, "{0}{0}// Enter Scope({1})",
                                 INDENT, id.index())?,
                    Annotation::ExitScope(id) =>
                        writeln!(w, "{0}{0}// Exit Scope({1})",
                                 INDENT, id.index())?,
                }
            }
        }

        writeln!(w, "{0}{0}{1:?}; // {2}",
                 INDENT,
                 statement,
                 comment(tcx, statement.scope, statement.span))?;

        current_location.statement_index += 1;
    }

    // Terminator at the bottom.
    writeln!(w, "{0}{0}{1:?}; // {2}",
             INDENT,
             data.terminator(),
             comment(tcx, data.terminator().scope, data.terminator().span))?;

    writeln!(w, "{}}}", INDENT)
}

fn comment(tcx: &TyCtxt,
           scope: ScopeId,
           span: Span)
           -> String {
    format!("Scope({}) at {}", scope.index(), tcx.sess.codemap().span_to_string(span))
}

fn write_scope_tree(tcx: &TyCtxt,
                    mir: &Mir,
                    auxiliary: Option<&Vec<ScopeAuxiliary>>,
                    scope_tree: &FnvHashMap<Option<ScopeId>, Vec<ScopeId>>,
                    w: &mut Write,
                    parent: Option<ScopeId>,
                    depth: usize)
                    -> io::Result<()> {
    for &child in scope_tree.get(&parent).unwrap_or(&vec![]) {
        let indent = depth * INDENT.len();
        let data = &mir.scopes[child];
        assert_eq!(data.parent_scope, parent);
        writeln!(w, "{0:1$}Scope({2}) {{", "", indent, child.index())?;

        let indent = indent + INDENT.len();
        if let Some(parent) = parent {
            writeln!(w, "{0:1$}Parent: Scope({2})", "", indent, parent.index())?;
        }

        if let Some(auxiliary) = auxiliary {
            let extent = auxiliary[child.index()].extent;
            let data = tcx.region_maps.code_extent_data(extent);
            writeln!(w, "{0:1$}Extent: {2:?}", "", indent, data)?;
        }

        write_scope_tree(tcx, mir, auxiliary, scope_tree, w,
                         Some(child), depth + 1)?;
    }
    Ok(())
}

/// Write out a human-readable textual representation of the MIR's `fn` type and the types of its
/// local variables (both user-defined bindings and compiler temporaries).
fn write_mir_intro(tcx: &TyCtxt, nid: NodeId, mir: &Mir, w: &mut Write)
                   -> io::Result<()> {
    write!(w, "fn {}(", tcx.map.path_to_string(nid))?;

    // fn argument types.
    for (i, arg) in mir.arg_decls.iter().enumerate() {
        if i > 0 {
            write!(w, ", ")?;
        }
        write!(w, "{:?}: {}", Lvalue::Arg(i as u32), arg.ty)?;
    }

    write!(w, ") -> ")?;

    // fn return type.
    match mir.return_ty {
        ty::FnOutput::FnConverging(ty) => write!(w, "{}", ty)?,
        ty::FnOutput::FnDiverging => write!(w, "!")?,
    }

    writeln!(w, " {{")?;

    // User variable types (including the user's name in a comment).
    for (i, var) in mir.var_decls.iter().enumerate() {
        write!(w, "{}let ", INDENT)?;
        if var.mutability == Mutability::Mut {
            write!(w, "mut ")?;
        }
        writeln!(w, "{:?}: {}; // {}", Lvalue::Var(i as u32), var.ty, var.name)?;
    }

    // Compiler-introduced temporary types.
    for (i, temp) in mir.temp_decls.iter().enumerate() {
        writeln!(w, "{}let mut {:?}: {};", INDENT, Lvalue::Temp(i as u32), temp.ty)?;
    }

    Ok(())
}
