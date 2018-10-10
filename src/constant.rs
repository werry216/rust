use cranelift_module::*;
use crate::prelude::*;
use crate::rustc::mir::interpret::{
    read_target_uint, AllocId, AllocType, Allocation, ConstValue, EvalResult, GlobalId, Scalar,
};
use crate::rustc::ty::Const;
use crate::rustc_mir::interpret::{EvalContext, Machine, Memory, MemoryKind, OpTy, PlaceTy};

#[derive(Default)]
pub struct ConstantCx {
    todo: HashSet<TodoItem>,
    done: HashSet<DataId>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
enum TodoItem {
    Alloc(AllocId),
    Static(DefId),
}

impl ConstantCx {
    pub fn finalize<'a, 'tcx: 'a, B: Backend>(
        mut self,
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        module: &mut Module<B>,
    ) {
        //println!("todo {:?}", self.todo);
        define_all_allocs(tcx, module, &mut self);
        //println!("done {:?}", self.done);
        self.done.clear();
    }
}

pub fn codegen_static<'a, 'tcx: 'a>(ccx: &mut ConstantCx, def_id: DefId) {
    ccx.todo.insert(TodoItem::Static(def_id));
}

pub fn codegen_static_ref<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    static_: &Static<'tcx>,
) -> CPlace<'tcx> {
    let data_id = data_id_for_static(fx.tcx, fx.module, static_.def_id);
    cplace_for_dataid(fx, static_.ty, data_id)
}

pub fn trans_promoted<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    promoted: Promoted,
) -> CPlace<'tcx> {
    let const_ = fx
        .tcx
        .const_eval(ParamEnv::reveal_all().and(GlobalId {
            instance: fx.instance,
            promoted: Some(promoted),
        })).unwrap();

    let const_ = force_eval_const(fx, const_);
    trans_const_place(fx, const_)
}

pub fn trans_constant<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    constant: &Constant<'tcx>,
) -> CValue<'tcx> {
    let const_ = fx.monomorphize(&constant.literal);
    let const_ = force_eval_const(fx, const_);
    trans_const_value(fx, const_)
}

pub fn force_eval_const<'a, 'tcx: 'a>(
    fx: &FunctionCx<'a, 'tcx, impl Backend>,
    const_: &'tcx Const<'tcx>,
) -> &'tcx Const<'tcx> {
    match const_.val {
        ConstValue::Unevaluated(def_id, ref substs) => {
            let param_env = ParamEnv::reveal_all();
            let instance = Instance::resolve(fx.tcx, param_env, def_id, substs).unwrap();
            let cid = GlobalId {
                instance,
                promoted: None,
            };
            fx.tcx.const_eval(param_env.and(cid)).unwrap()
        }
        _ => const_,
    }
}

fn trans_const_value<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    const_: &'tcx Const<'tcx>,
) -> CValue<'tcx> {
    let ty = fx.monomorphize(&const_.ty);
    let layout = fx.layout_of(ty);
    match ty.sty {
        ty::Bool => {
            let bits = const_.val.try_to_bits(layout.size).unwrap();
            CValue::const_val(fx, ty, bits as u64 as i64)
        }
        ty::Uint(_) => {
            let bits = const_.val.try_to_bits(layout.size).unwrap();
            CValue::const_val(fx, ty, bits as u64 as i64)
        }
        ty::Int(_) => {
            let bits = const_.val.try_to_bits(layout.size).unwrap();
            CValue::const_val(fx, ty, bits as i128 as i64)
        }
        ty::FnDef(def_id, substs) => {
            let func_ref = fx.get_function_ref(
                Instance::resolve(fx.tcx, ParamEnv::reveal_all(), def_id, substs).unwrap(),
            );
            let func_addr = fx.bcx.ins().func_addr(fx.module.pointer_type(), func_ref);
            CValue::ByVal(func_addr, layout)
        }
        _ => trans_const_place(fx, const_).to_cvalue(fx),
    }
}

fn trans_const_place<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    const_: &'tcx Const<'tcx>,
) -> CPlace<'tcx> {
    // Adapted from https://github.com/rust-lang/rust/pull/53671/files#diff-e0b58bb6712edaa8595ad7237542c958L551
    let result = || -> EvalResult<'tcx, &'tcx Allocation> {
        let mut ecx = EvalContext::new(
            fx.tcx.at(DUMMY_SP),
            ty::ParamEnv::reveal_all(),
            TransPlaceInterpreter,
            (),
        );
        let op = ecx.const_to_op(const_)?;
        let ptr = ecx.allocate(op.layout, MemoryKind::Stack)?;
        ecx.copy_op(op, ptr.into())?;
        let alloc = ecx.memory.get(ptr.to_ptr()?.alloc_id)?;
        Ok(fx.tcx.intern_const_alloc(alloc.clone()))
    };
    let alloc = result().expect("unable to convert ConstValue to Allocation");

    //println!("const value: {:?} allocation: {:?}", value, alloc);
    let alloc_id = fx.tcx.alloc_map.lock().allocate(alloc);
    fx.constants.todo.insert(TodoItem::Alloc(alloc_id));
    let data_id = data_id_for_alloc_id(fx.module, alloc_id);
    cplace_for_dataid(fx, const_.ty, data_id)
}

fn data_id_for_alloc_id<B: Backend>(module: &mut Module<B>, alloc_id: AllocId) -> DataId {
    module
        .declare_data(&alloc_id.0.to_string(), Linkage::Local, false)
        .unwrap()
}

fn data_id_for_static<'a, 'tcx: 'a, B: Backend>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    module: &mut Module<B>,
    def_id: DefId,
) -> DataId {
    let symbol_name = tcx.symbol_name(Instance::mono(tcx, def_id)).as_str();
    let is_mutable =
        if let crate::rustc::hir::Mutability::MutMutable = tcx.is_static(def_id).unwrap() {
            true
        } else {
            !tcx.type_of(def_id)
                .is_freeze(tcx, ParamEnv::reveal_all(), DUMMY_SP)
        };
    module
        .declare_data(&*symbol_name, Linkage::Export, is_mutable)
        .unwrap()
}

fn cplace_for_dataid<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    ty: Ty<'tcx>,
    data_id: DataId,
) -> CPlace<'tcx> {
    let local_data_id = fx.module.declare_data_in_func(data_id, &mut fx.bcx.func);
    let global_ptr = fx
        .bcx
        .ins()
        .global_value(fx.module.pointer_type(), local_data_id);
    let layout = fx.layout_of(fx.monomorphize(&ty));
    assert!(!layout.is_unsized(), "unsized statics aren't supported");
    CPlace::Addr(global_ptr, None, layout)
}

fn define_all_allocs<'a, 'tcx: 'a, B: Backend + 'a>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    module: &mut Module<B>,
    cx: &mut ConstantCx,
) {
    let memory = Memory::<TransPlaceInterpreter>::new(tcx.at(DUMMY_SP), ());

    while let Some(todo_item) = pop_set(&mut cx.todo) {
        let (data_id, alloc) = match todo_item {
            TodoItem::Alloc(alloc_id) => {
                //println!("alloc_id {}", alloc_id);
                let data_id = data_id_for_alloc_id(module, alloc_id);
                let alloc = memory.get(alloc_id).unwrap();
                (data_id, alloc)
            }
            TodoItem::Static(def_id) => {
                //println!("static {:?}", def_id);
                let instance = ty::Instance::mono(tcx, def_id);
                let cid = GlobalId {
                    instance,
                    promoted: None,
                };
                let const_ = tcx.const_eval(ParamEnv::reveal_all().and(cid)).unwrap();

                let alloc = match const_.val {
                    ConstValue::ByRef(_alloc_id, alloc, n) if n.bytes() == 0 => alloc,
                    _ => bug!("static const eval returned {:#?}", const_),
                };

                let data_id = data_id_for_static(tcx, module, def_id);
                (data_id, alloc)
            }
        };

        //("data_id {}", data_id);
        if cx.done.contains(&data_id) {
            continue;
        }

        let mut data_ctx = DataContext::new();

        data_ctx.define(alloc.bytes.to_vec().into_boxed_slice());

        for &(offset, reloc) in alloc.relocations.iter() {
            let reloc_offset = {
                let endianness = tcx.data_layout.endian;
                let offset = offset.bytes() as usize;
                let ptr_size = tcx.data_layout.pointer_size;
                let bytes = &alloc.bytes[offset..offset + ptr_size.bytes() as usize];
                read_target_uint(endianness, bytes).unwrap()
            };

            let data_id = match tcx.alloc_map.lock().get(reloc).unwrap() {
                AllocType::Function(instance) => {
                    let (func_name, sig) = crate::abi::get_function_name_and_sig(tcx, instance);
                    let func_id = module
                        .declare_function(&func_name, Linkage::Import, &sig)
                        .unwrap();
                    let local_func_id = module.declare_func_in_data(func_id, &mut data_ctx);
                    data_ctx.write_function_addr(reloc_offset as u32, local_func_id);
                    continue;
                }
                AllocType::Memory(_) => {
                    cx.todo.insert(TodoItem::Alloc(reloc));
                    data_id_for_alloc_id(module, reloc)
                }
                AllocType::Static(def_id) => {
                    cx.todo.insert(TodoItem::Static(def_id));
                    data_id_for_static(tcx, module, def_id)
                }
            };

            let global_value = module.declare_data_in_data(data_id, &mut data_ctx);
            data_ctx.write_data_addr(reloc_offset as u32, global_value, 0);
        }

        module.define_data(data_id, &data_ctx).unwrap();
        cx.done.insert(data_id);
    }

    assert!(cx.todo.is_empty(), "{:?}", cx.todo);
}

fn pop_set<T: Copy + Eq + ::std::hash::Hash>(set: &mut HashSet<T>) -> Option<T> {
    if let Some(elem) = set.iter().next().map(|elem| *elem) {
        set.remove(&elem);
        Some(elem)
    } else {
        None
    }
}

struct TransPlaceInterpreter;

impl<'a, 'mir, 'tcx> Machine<'a, 'mir, 'tcx> for TransPlaceInterpreter {
    type MemoryData = ();
    type MemoryKinds = ();
    const MUT_STATIC_KIND: Option<()> = None;
    const ENFORCE_VALIDITY: bool = true;

    fn before_terminator(_: &mut EvalContext<'a, 'mir, 'tcx, Self>) -> EvalResult<'tcx> {
        panic!();
    }

    fn find_fn(
        _: &mut EvalContext<'a, 'mir, 'tcx, Self>,
        _: Instance<'tcx>,
        _: &[OpTy<'tcx>],
        _: Option<PlaceTy<'tcx>>,
        _: Option<BasicBlock>,
    ) -> EvalResult<'tcx, Option<&'mir Mir<'tcx>>> {
        panic!();
    }

    fn call_intrinsic(
        _: &mut EvalContext<'a, 'mir, 'tcx, Self>,
        _: Instance<'tcx>,
        _: &[OpTy<'tcx>],
        _: PlaceTy<'tcx>,
    ) -> EvalResult<'tcx> {
        panic!();
    }

    fn find_foreign_static(
        _: crate::rustc::ty::query::TyCtxtAt<'a, 'tcx, 'tcx>,
        _: DefId,
    ) -> EvalResult<'tcx, &'tcx Allocation> {
        panic!();
    }

    fn ptr_op(
        _: &EvalContext<'a, 'mir, 'tcx, Self>,
        _: mir::BinOp,
        _: Scalar,
        _: TyLayout<'tcx>,
        _: Scalar,
        _: TyLayout<'tcx>,
    ) -> EvalResult<'tcx, (Scalar, bool)> {
        panic!();
    }

    fn box_alloc(_: &mut EvalContext<'a, 'mir, 'tcx, Self>, _: PlaceTy<'tcx>) -> EvalResult<'tcx> {
        panic!();
    }
}
