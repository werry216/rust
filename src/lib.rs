#![feature(rustc_private, macro_at_most_once_rep)]
#![allow(intra_doc_link_resolution_failure)]

extern crate byteorder;
extern crate syntax;
#[macro_use]
extern crate rustc;
extern crate rustc_codegen_utils;
extern crate rustc_incremental;
extern crate rustc_mir;
extern crate rustc_target;
#[macro_use]
extern crate rustc_data_structures;

extern crate ar;
#[macro_use]
extern crate bitflags;
extern crate faerie;
//extern crate goblin;
extern crate cranelift;
extern crate cranelift_faerie;
extern crate cranelift_module;
extern crate cranelift_simplejit;
extern crate target_lexicon;

use std::any::Any;
use std::fs::File;
use std::sync::{mpsc, Arc};

use crate::rustc::dep_graph::DepGraph;
use crate::rustc::middle::cstore::MetadataLoader;
use crate::rustc::session::{config::OutputFilenames, CompileIncomplete};
use crate::rustc::ty::query::Providers;
use crate::rustc_codegen_utils::codegen_backend::CodegenBackend;
use crate::rustc_codegen_utils::link::out_filename;
use crate::rustc_data_structures::svh::Svh;
use crate::syntax::symbol::Symbol;

use cranelift::codegen::settings;
use cranelift_faerie::*;

struct NonFatal(pub String);

macro_rules! unimpl {
    ($($tt:tt)*) => {
        panic!(crate::NonFatal(format!($($tt)*)));
    };
}

mod abi;
mod analyze;
mod base;
mod common;
mod constant;
mod intrinsics;
mod metadata;
mod pretty_clif;
mod vtable;

mod prelude {
    pub use std::any::Any;
    pub use std::collections::{HashMap, HashSet};

    pub use crate::rustc::hir::def_id::{DefId, LOCAL_CRATE};
    pub use crate::rustc::mir;
    pub use crate::rustc::mir::interpret::AllocId;
    pub use crate::rustc::mir::*;
    pub use crate::rustc::session::{config::CrateType, Session};
    pub use crate::rustc::ty::layout::{self, Abi, LayoutOf, Scalar, Size, TyLayout};
    pub use crate::rustc::ty::{
        self, subst::Substs, FnSig, Instance, InstanceDef, ParamEnv, PolyFnSig, Ty, TyCtxt,
        TypeAndMut, TypeFoldable,
    };
    pub use crate::rustc_data_structures::{
        fx::{FxHashMap, FxHashSet},
        indexed_vec::Idx,
        sync::Lrc,
    };
    pub use crate::rustc_mir::monomorphize::{collector, MonoItem};
    pub use crate::syntax::ast::{FloatTy, IntTy, UintTy};
    pub use crate::syntax::source_map::DUMMY_SP;

    pub use cranelift::codegen::ir::{
        condcodes::IntCC, function::Function, ExternalName, FuncRef, Inst, StackSlot,
    };
    pub use cranelift::codegen::Context;
    pub use cranelift::prelude::*;
    pub use cranelift_module::{Backend, DataContext, DataId, FuncId, Linkage, Module};
    pub use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};

    pub use crate::abi::*;
    pub use crate::base::{trans_operand, trans_place};
    pub use crate::common::*;
    pub use crate::Caches;

    pub fn should_codegen(sess: &Session) -> bool {
        //return true;
        ::std::env::var("SHOULD_CODEGEN").is_ok()
            || sess.crate_types.get().contains(&CrateType::Executable)
    }
}

use crate::constant::ConstantCx;
use crate::prelude::*;

pub struct Caches<'tcx> {
    pub context: Context,
    pub vtables: HashMap<(Ty<'tcx>, Option<ty::PolyExistentialTraitRef<'tcx>>), DataId>,
}

impl<'tcx> Caches<'tcx> {
    fn new() -> Self {
        Caches {
            context: Context::new(),
            vtables: HashMap::new(),
        }
    }
}

struct CraneliftCodegenBackend;

struct OngoingCodegen {
    product: cranelift_faerie::FaerieProduct,
    metadata: Vec<u8>,
    crate_name: Symbol,
    crate_hash: Svh,
}

impl CodegenBackend for CraneliftCodegenBackend {
    fn init(&self, sess: &Session) {
        for cty in sess.opts.crate_types.iter() {
            match *cty {
                CrateType::Rlib | CrateType::Dylib | CrateType::Executable => {}
                _ => {
                    sess.err(&format!(
                        "Rustc codegen cranelift doesn't support output type {}",
                        cty
                    ));
                }
            }
        }
    }

    fn metadata_loader(&self) -> Box<MetadataLoader + Sync> {
        Box::new(crate::metadata::CraneliftMetadataLoader)
    }

    fn provide(&self, providers: &mut Providers) {
        rustc_codegen_utils::symbol_names::provide(providers);

        providers.target_features_whitelist = |_tcx, _cnum| Lrc::new(Default::default());
        providers.is_reachable_non_generic = |_tcx, _defid| true;
        providers.exported_symbols = |_tcx, _crate| Arc::new(Vec::new());
        providers.upstream_monomorphizations = |_tcx, _cnum| Lrc::new(FxHashMap());
        providers.upstream_monomorphizations_for = |tcx, def_id| {
            debug_assert!(!def_id.is_local());
            tcx.upstream_monomorphizations(LOCAL_CRATE)
                .get(&def_id)
                .cloned()
        };
    }
    fn provide_extern(&self, providers: &mut Providers) {
        providers.is_reachable_non_generic = |_tcx, _defid| true;
    }

    fn codegen_crate<'a, 'tcx>(
        &self,
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        _rx: mpsc::Receiver<Box<Any + Send>>,
    ) -> Box<Any> {
        if !tcx.sess.crate_types.get().contains(&CrateType::Executable)
            && std::env::var("SHOULD_RUN").is_ok()
        {
            tcx.sess
                .err("Can't JIT run non executable (SHOULD_RUN env var is set)");
        }

        tcx.sess.abort_if_errors();

        let metadata = tcx.encode_metadata();

        fn build_isa(tcx: TyCtxt) -> Box<isa::TargetIsa> {
            let mut flags_builder = settings::builder();
            flags_builder.enable("is_pic").unwrap();
            let flags = settings::Flags::new(flags_builder);
            cranelift::codegen::isa::lookup(tcx.sess.target.target.llvm_target.parse().unwrap())
                .unwrap()
                .finish(flags)
        }

        let isa = build_isa(tcx);

        let mono_items =
            collector::collect_crate_mono_items(tcx, collector::MonoItemCollectionMode::Eager).0;

        // TODO: move to the end of this function when compiling libcore doesn't have unimplemented stuff anymore
        save_incremental(tcx);
        tcx.sess.warn("Saved incremental data");

        if std::env::var("SHOULD_RUN").is_ok() {
            let mut jit_module: Module<SimpleJITBackend> = Module::new(SimpleJITBuilder::new());
            assert_eq!(pointer_ty(tcx), jit_module.pointer_type());

            codegen_mono_items(tcx, &*isa, &mut jit_module, &mono_items);

            tcx.sess.abort_if_errors();
            println!("Compiled everything");
            println!("Rustc codegen cranelift will JIT run the executable, because the SHOULD_RUN env var is set");

            let sig = Signature {
                params: vec![
                    AbiParam::new(jit_module.pointer_type()),
                    AbiParam::new(jit_module.pointer_type()),
                ],
                returns: vec![AbiParam::new(jit_module.pointer_type() /*isize*/)],
                call_conv: CallConv::SystemV,
            };
            let main_func_id = jit_module
                .declare_function("main", Linkage::Import, &sig)
                .unwrap();

            jit_module.finalize_definitions();
            let finalized_main: *const u8 = jit_module.get_finalized_function(main_func_id);
            println!("🎉 Finalized everything");

            let f: extern "C" fn(isize, *const *const u8) -> isize =
                unsafe { ::std::mem::transmute(finalized_main) };
            let res = f(0, 0 as *const _);
            tcx.sess.warn(&format!("🚀 main returned {}", res));

            jit_module.finish();
            ::std::process::exit(0);
        } else {
            let mut faerie_module: Module<FaerieBackend> = Module::new(
                FaerieBuilder::new(
                    isa,
                    "some_file.o".to_string(),
                    FaerieTrapCollection::Disabled,
                    FaerieBuilder::default_libcall_names(),
                ).unwrap(),
            );
            assert_eq!(pointer_ty(tcx), faerie_module.pointer_type());

            codegen_mono_items(tcx, &*build_isa(tcx), &mut faerie_module, &mono_items);

            tcx.sess.abort_if_errors();

            if should_codegen(tcx.sess) {
                faerie_module.finalize_definitions();
            }

            return Box::new(OngoingCodegen {
                product: faerie_module.finish(),
                metadata: metadata.raw_data,
                crate_name: tcx.crate_name(LOCAL_CRATE),
                crate_hash: tcx.crate_hash(LOCAL_CRATE),
            });
        }
    }

    fn join_codegen_and_link(
        &self,
        ongoing_codegen: Box<Any>,
        sess: &Session,
        _dep_graph: &DepGraph,
        outputs: &OutputFilenames,
    ) -> Result<(), CompileIncomplete> {
        let ongoing_codegen = *ongoing_codegen
            .downcast::<OngoingCodegen>()
            .expect("Expected CraneliftCodegenBackend's OngoingCodegen, found Box<Any>");

        let mut artifact = ongoing_codegen.product.artifact;
        let metadata = ongoing_codegen.metadata;

        let metadata_name =
            ".rustc.clif_metadata".to_string() + &ongoing_codegen.crate_hash.to_string();
        artifact
            .declare_with(
                &metadata_name,
                faerie::artifact::Decl::Data {
                    global: true,
                    writable: false,
                },
                metadata.clone(),
            ).unwrap();

        for &crate_type in sess.opts.crate_types.iter() {
            match crate_type {
                // TODO: link executable
                CrateType::Executable | CrateType::Rlib => {
                    let output_name = out_filename(
                        sess,
                        crate_type,
                        &outputs,
                        &ongoing_codegen.crate_name.as_str(),
                    );
                    let file = File::create(&output_name).unwrap();
                    let mut builder = ar::Builder::new(file);
                    builder
                        .append(
                            &ar::Header::new(
                                metadata_name.as_bytes().to_vec(),
                                metadata.len() as u64,
                            ),
                            ::std::io::Cursor::new(metadata.clone()),
                        ).unwrap();
                    if should_codegen(sess) {
                        let obj = artifact.emit().unwrap();
                        builder
                            .append(
                                &ar::Header::new(b"data.o".to_vec(), obj.len() as u64),
                                ::std::io::Cursor::new(obj),
                            ).unwrap();
                    }
                }
                _ => sess.fatal(&format!("Unsupported crate type: {:?}", crate_type)),
            }
        }
        Ok(())
    }
}

fn codegen_mono_items<'a, 'tcx: 'a>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    isa: &isa::TargetIsa,
    module: &mut Module<impl Backend + 'static>,
    mono_items: &FxHashSet<MonoItem<'tcx>>,
) {
    use std::io::Write;

    let mut caches = Caches::new();
    let mut ccx = ConstantCx::default();

    let mut log = if cfg!(debug_assertions) {
        Some(::std::fs::File::create(concat!(env!("CARGO_MANIFEST_DIR"), "/target/out/log.txt")).unwrap())
    } else {
        None
    };

    let before = ::std::time::Instant::now();
    println!("[codegen mono items] start");

    for mono_item in mono_items {
        let res = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
            base::trans_mono_item(tcx, isa, module, &mut caches, &mut ccx, *mono_item);
        }));

        if let Err(err) = res {
            match err.downcast::<NonFatal>() {
                Ok(non_fatal) => {
                    if cfg!(debug_assertions) {
                        writeln!(log.as_mut().unwrap(), "{}", &non_fatal.0);
                    }
                    tcx.sess.err(&non_fatal.0)
                }
                Err(err) => ::std::panic::resume_unwind(err),
            }
        }
    }

    maybe_create_entry_wrapper(tcx, module);

    ccx.finalize(tcx, module);

    let after = ::std::time::Instant::now();
    println!("[codegen mono items] end time: {:?}", after - before);
}

fn save_incremental<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>) {
    rustc_incremental::assert_dep_graph(tcx);
    rustc_incremental::save_dep_graph(tcx);
    rustc_incremental::finalize_session_directory(tcx.sess, tcx.crate_hash(LOCAL_CRATE));
}

/// This is the entrypoint for a hot plugged rustc_codegen_cranelift
#[no_mangle]
pub fn __rustc_codegen_backend() -> Box<CodegenBackend> {
    Box::new(CraneliftCodegenBackend)
}

/// Create the `main` function which will initialize the rust runtime and call
/// users main function.
fn maybe_create_entry_wrapper<'a, 'tcx: 'a>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    module: &mut Module<impl Backend + 'static>,
) {
    use crate::rustc::middle::lang_items::StartFnLangItem;
    use crate::rustc::session::config::EntryFnType;

    let (main_def_id, use_start_lang_item) = match *tcx.sess.entry_fn.borrow() {
        Some((id, _, entry_ty)) => (
            tcx.hir.local_def_id(id),
            match entry_ty {
                EntryFnType::Main => true,
                EntryFnType::Start => false,
            },
        ),
        None => return,
    };

    create_entry_fn(tcx, module, main_def_id, use_start_lang_item);;

    fn create_entry_fn<'a, 'tcx: 'a>(
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        m: &mut Module<impl Backend + 'static>,
        rust_main_def_id: DefId,
        use_start_lang_item: bool,
    ) {
        let main_ret_ty = tcx.fn_sig(rust_main_def_id).output();
        // Given that `main()` has no arguments,
        // then its return type cannot have
        // late-bound regions, since late-bound
        // regions must appear in the argument
        // listing.
        let main_ret_ty = tcx.erase_regions(&main_ret_ty.no_late_bound_regions().unwrap());

        let cmain_sig = Signature {
            params: vec![
                AbiParam::new(m.pointer_type()),
                AbiParam::new(m.pointer_type()),
            ],
            returns: vec![AbiParam::new(m.pointer_type() /*isize*/)],
            call_conv: CallConv::SystemV,
        };

        let cmain_func_id = m
            .declare_function("main", Linkage::Export, &cmain_sig)
            .unwrap();

        let instance = Instance::mono(tcx, rust_main_def_id);

        let (main_name, main_sig) = get_function_name_and_sig(tcx, instance);

        let main_func_id = m
            .declare_function(&main_name, Linkage::Import, &main_sig)
            .unwrap();

        let mut ctx = Context::new();
        ctx.func = Function::with_name_signature(ExternalName::user(0, 0), cmain_sig.clone());
        {
            let mut func_ctx = FunctionBuilderContext::new();
            let mut bcx: FunctionBuilder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

            let ebb = bcx.create_ebb();
            bcx.switch_to_block(ebb);
            let arg_argc = bcx.append_ebb_param(ebb, m.pointer_type());
            let arg_argv = bcx.append_ebb_param(ebb, m.pointer_type());

            let main_func_ref = m.declare_func_in_func(main_func_id, &mut bcx.func);

            let call_inst = if use_start_lang_item {
                let start_def_id = tcx.require_lang_item(StartFnLangItem);
                let start_instance = Instance::resolve(
                    tcx,
                    ParamEnv::reveal_all(),
                    start_def_id,
                    tcx.intern_substs(&[main_ret_ty.into()]),
                ).unwrap();

                let (start_name, start_sig) = get_function_name_and_sig(tcx, start_instance);
                let start_func_id = m
                    .declare_function(&start_name, Linkage::Import, &start_sig)
                    .unwrap();

                let main_val = bcx.ins().func_addr(m.pointer_type(), main_func_ref);

                let func_ref = m.declare_func_in_func(start_func_id, &mut bcx.func);
                bcx.ins().call(func_ref, &[main_val, arg_argc, arg_argv])
            } else {
                // using user-defined start fn
                bcx.ins().call(main_func_ref, &[arg_argc, arg_argv])
            };

            let result = bcx.inst_results(call_inst)[0];
            bcx.ins().return_(&[result]);
            bcx.seal_all_blocks();
            bcx.finalize();
        }
        m.define_function(cmain_func_id, &mut ctx).unwrap();
    }
}
