// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::ty::layout::{HasTyCtxt, LayoutOf, TyLayout};
use rustc::ty::Ty;

use super::CodegenObject;
use rustc::middle::allocator::AllocatorKind;
use rustc::middle::cstore::EncodedMetadata;
use rustc::mir::mono::Stats;
use rustc::session::Session;
use rustc::ty::TyCtxt;
use rustc::util::time_graph::TimeGraph;
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use std::any::Any;
use std::sync::mpsc::Receiver;
use syntax_pos::symbol::InternedString;
use {CachedModuleCodegen, ModuleCodegen};

pub trait BackendTypes {
    type Value: CodegenObject;
    type BasicBlock: Copy;
    type Type: CodegenObject;
    type Context;
    type Funclet;

    type DIScope: Copy;
}

pub trait Backend<'tcx>:
    Sized + BackendTypes + HasTyCtxt<'tcx> + LayoutOf<Ty = Ty<'tcx>, TyLayout = TyLayout<'tcx>>
{
}

impl<'tcx, T> Backend<'tcx> for T where
    Self: BackendTypes + HasTyCtxt<'tcx> + LayoutOf<Ty = Ty<'tcx>, TyLayout = TyLayout<'tcx>>
{}

pub trait ExtraBackendMethods: CodegenBackend {
    type Module;
    type OngoingCodegen;

    fn new_metadata(&self, sess: &Session, mod_name: &str) -> Self::Module;
    fn write_metadata<'b, 'gcx>(
        &self,
        tcx: TyCtxt<'b, 'gcx, 'gcx>,
        metadata: &Self::Module,
    ) -> EncodedMetadata;
    fn codegen_allocator(&self, tcx: TyCtxt, mods: &Self::Module, kind: AllocatorKind);

    fn start_async_codegen(
        &self,
        tcx: TyCtxt,
        time_graph: Option<TimeGraph>,
        metadata: EncodedMetadata,
        coordinator_receive: Receiver<Box<dyn Any + Send>>,
        total_cgus: usize,
    ) -> Self::OngoingCodegen;
    fn submit_pre_codegened_module_to_backend(
        &self,
        codegen: &Self::OngoingCodegen,
        tcx: TyCtxt,
        module: ModuleCodegen<Self::Module>,
    );
    fn submit_pre_lto_module_to_backend(&self, tcx: TyCtxt, module: CachedModuleCodegen);
    fn submit_post_lto_module_to_backend(&self, tcx: TyCtxt, module: CachedModuleCodegen);
    fn codegen_aborted(codegen: Self::OngoingCodegen);
    fn codegen_finished(&self, codegen: &Self::OngoingCodegen, tcx: TyCtxt);
    fn check_for_errors(&self, codegen: &Self::OngoingCodegen, sess: &Session);
    fn wait_for_signal_to_codegen_item(&self, codegen: &Self::OngoingCodegen);
    fn compile_codegen_unit<'a, 'tcx: 'a>(
        &self,
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        cgu_name: InternedString,
    ) -> Stats;
}
