
use driver::session;
use middle::trans::base;
use middle::trans::common::{T_fn, T_i1, T_i8, T_i32,
                               T_int, T_nil,
                               T_opaque_vec, T_ptr, T_unique_ptr,
                               T_size_t, T_void, T_vec2};
use lib::llvm::{type_names, ModuleRef, ValueRef, TypeRef};

type upcalls =
    {trace: ValueRef,
     malloc: ValueRef,
     free: ValueRef,
     exchange_malloc: ValueRef,
     exchange_free: ValueRef,
     validate_box: ValueRef,
     log_type: ValueRef,
     call_shim_on_c_stack: ValueRef,
     call_shim_on_rust_stack: ValueRef,
     rust_personality: ValueRef,
     reset_stack_limit: ValueRef};

fn declare_upcalls(targ_cfg: @session::config,
                   _tn: type_names,
                   tydesc_type: TypeRef,
                   llmod: ModuleRef) -> @upcalls {
    fn decl(llmod: ModuleRef, prefix: ~str, name: ~str,
            tys: ~[TypeRef], rv: TypeRef) ->
       ValueRef {
        let mut arg_tys: ~[TypeRef] = ~[];
        for tys.each |t| { arg_tys.push(*t); }
        let fn_ty = T_fn(arg_tys, rv);
        return base::decl_cdecl_fn(llmod, prefix + name, fn_ty);
    }
    fn nothrow(f: ValueRef) -> ValueRef {
        base::set_no_unwind(f); f
    }
    let d = |a,b,c| decl(llmod, ~"upcall_", a, b, c);
    let dv = |a,b| decl(llmod, ~"upcall_", a, b, T_void());

    let int_t = T_int(targ_cfg);

    return @{trace: dv(~"trace", ~[T_ptr(T_i8()),
                              T_ptr(T_i8()),
                              int_t]),
          malloc:
              nothrow(d(~"malloc",
                        ~[T_ptr(tydesc_type), int_t],
                        T_ptr(T_i8()))),
          free:
              nothrow(dv(~"free", ~[T_ptr(T_i8())])),
          exchange_malloc:
              nothrow(d(~"exchange_malloc",
                        ~[T_ptr(tydesc_type), int_t],
                        T_ptr(T_i8()))),
          exchange_free:
              nothrow(dv(~"exchange_free", ~[T_ptr(T_i8())])),
          validate_box:
              nothrow(dv(~"validate_box", ~[T_ptr(T_i8())])),
          log_type:
              dv(~"log_type", ~[T_ptr(tydesc_type),
                              T_ptr(T_i8()), T_i32()]),
          call_shim_on_c_stack:
              d(~"call_shim_on_c_stack",
                // arguments: void *args, void *fn_ptr
                ~[T_ptr(T_i8()), T_ptr(T_i8())],
                int_t),
          call_shim_on_rust_stack:
              d(~"call_shim_on_rust_stack",
                ~[T_ptr(T_i8()), T_ptr(T_i8())], int_t),
          rust_personality:
              nothrow(d(~"rust_personality", ~[], T_i32())),
          reset_stack_limit:
              nothrow(dv(~"reset_stack_limit", ~[]))
         };
}
//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
//
