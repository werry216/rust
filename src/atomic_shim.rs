//! Atomic intrinsics are implemented using a global lock for now, as Cranelift doesn't support
//! atomic operations yet.

// FIXME implement atomic instructions in Cranelift.

use crate::prelude::*;

#[cfg(feature = "jit")]
#[no_mangle]
pub static mut __cg_clif_global_atomic_mutex: libc::pthread_mutex_t = libc::PTHREAD_MUTEX_INITIALIZER;

pub(crate) fn init_global_lock(module: &mut Module<impl Backend>, bcx: &mut FunctionBuilder<'_>) {
    if std::env::var("CG_CLIF_JIT").is_ok() {
        // When using JIT, dylibs won't find the __cg_clif_global_atomic_mutex data object defined here,
        // so instead define it in the cg_clif dylib.

        return;
    }

    let mut data_ctx = DataContext::new();
    data_ctx.define_zeroinit(1024); // 1024 bytes should be big enough on all platforms.
    let atomic_mutex = module.declare_data(
        "__cg_clif_global_atomic_mutex",
        Linkage::Export,
        true,
        false,
        Some(16),
    ).unwrap();
    module.define_data(atomic_mutex, &data_ctx).unwrap();

    let pthread_mutex_init = module.declare_function("pthread_mutex_init", Linkage::Import, &cranelift_codegen::ir::Signature {
        call_conv: module.target_config().default_call_conv,
        params: vec![
            AbiParam::new(module.target_config().pointer_type() /* *mut pthread_mutex_t */),
            AbiParam::new(module.target_config().pointer_type() /* *const pthread_mutex_attr_t */),
        ],
        returns: vec![AbiParam::new(types::I32 /* c_int */)],
    }).unwrap();

    let pthread_mutex_init = module.declare_func_in_func(pthread_mutex_init, bcx.func);

    let atomic_mutex = module.declare_data_in_func(atomic_mutex, bcx.func);
    let atomic_mutex = bcx.ins().global_value(module.target_config().pointer_type(), atomic_mutex);

    let nullptr = bcx.ins().iconst(module.target_config().pointer_type(), 0);

    bcx.ins().call(pthread_mutex_init, &[atomic_mutex, nullptr]);
}

pub(crate) fn init_global_lock_constructor(
    module: &mut Module<impl Backend>,
    constructor_name: &str
) -> FuncId {
    let sig = Signature::new(CallConv::SystemV);
    let init_func_id = module
        .declare_function(constructor_name, Linkage::Export, &sig)
        .unwrap();

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(ExternalName::user(0, 0), sig);
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

        let block = bcx.create_block();
        bcx.switch_to_block(block);

        crate::atomic_shim::init_global_lock(module, &mut bcx);

        bcx.ins().return_(&[]);
        bcx.seal_all_blocks();
        bcx.finalize();
    }
    module.define_function(
        init_func_id,
        &mut ctx,
        &mut cranelift_codegen::binemit::NullTrapSink {},
    ).unwrap();

    init_func_id
}

pub(crate) fn lock_global_lock(fx: &mut FunctionCx<'_, '_, impl Backend>) {
    let atomic_mutex = fxcodegen_cx.module.declare_data(
        "__cg_clif_global_atomic_mutex",
        Linkage::Import,
        true,
        false,
        None,
    ).unwrap();

    let pthread_mutex_lock = fxcodegen_cx.module.declare_function("pthread_mutex_lock", Linkage::Import, &cranelift_codegen::ir::Signature {
        call_conv: fxcodegen_cx.module.target_config().default_call_conv,
        params: vec![
            AbiParam::new(fxcodegen_cx.module.target_config().pointer_type() /* *mut pthread_mutex_t */),
        ],
        returns: vec![AbiParam::new(types::I32 /* c_int */)],
    }).unwrap();

    let pthread_mutex_lock = fxcodegen_cx.module.declare_func_in_func(pthread_mutex_lock, fx.bcx.func);

    let atomic_mutex = fxcodegen_cx.module.declare_data_in_func(atomic_mutex, fx.bcx.func);
    let atomic_mutex = fx.bcx.ins().global_value(fxcodegen_cx.module.target_config().pointer_type(), atomic_mutex);

    fx.bcx.ins().call(pthread_mutex_lock, &[atomic_mutex]);
}

pub(crate) fn unlock_global_lock(fx: &mut FunctionCx<'_, '_, impl Backend>) {
    let atomic_mutex = fxcodegen_cx.module.declare_data(
        "__cg_clif_global_atomic_mutex",
        Linkage::Import,
        true,
        false,
        None,
    ).unwrap();

    let pthread_mutex_unlock = fxcodegen_cx.module.declare_function("pthread_mutex_unlock", Linkage::Import, &cranelift_codegen::ir::Signature {
        call_conv: fxcodegen_cx.module.target_config().default_call_conv,
        params: vec![
            AbiParam::new(fxcodegen_cx.module.target_config().pointer_type() /* *mut pthread_mutex_t */),
        ],
        returns: vec![AbiParam::new(types::I32 /* c_int */)],
    }).unwrap();

    let pthread_mutex_unlock = fxcodegen_cx.module.declare_func_in_func(pthread_mutex_unlock, fx.bcx.func);

    let atomic_mutex = fxcodegen_cx.module.declare_data_in_func(atomic_mutex, fx.bcx.func);
    let atomic_mutex = fx.bcx.ins().global_value(fxcodegen_cx.module.target_config().pointer_type(), atomic_mutex);

    fx.bcx.ins().call(pthread_mutex_unlock, &[atomic_mutex]);
}
