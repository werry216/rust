use std::fmt;

use rustc_target::spec::{HasTargetSpec, Target};

use cranelift_module::{DataId, Module};

use crate::prelude::*;

pub type CurrentBackend = ::cranelift_simplejit::SimpleJITBackend;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Variable(pub Local);

impl EntityRef for Variable {
    fn new(u: usize) -> Self {
        Variable(Local::new(u))
    }

    fn index(self) -> usize {
        self.0.index()
    }
}

pub fn cton_type_from_ty<'a, 'tcx: 'a>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    ty: Ty<'tcx>,
) -> Option<types::Type> {
    Some(match ty.sty {
        TypeVariants::TyBool => types::I8,
        TypeVariants::TyUint(size) => match size {
            UintTy::U8 => types::I8,
            UintTy::U16 => types::I16,
            UintTy::U32 => types::I32,
            UintTy::U64 => types::I64,
            UintTy::U128 => unimplemented!("u128"),
            UintTy::Usize => types::I64,
        },
        TypeVariants::TyInt(size) => match size {
            IntTy::I8 => types::I8,
            IntTy::I16 => types::I16,
            IntTy::I32 => types::I32,
            IntTy::I64 => types::I64,
            IntTy::I128 => unimplemented!("i128"),
            IntTy::Isize => types::I64,
        },
        TypeVariants::TyChar => types::I32,
        TypeVariants::TyFloat(size) => match size {
            FloatTy::F32 => types::F32,
            FloatTy::F64 => types::F64,
        },
        TypeVariants::TyFnPtr(_) => types::I64,
        TypeVariants::TyRawPtr(TypeAndMut { ty, mutbl: _ }) | TypeVariants::TyRef(_, ty, _) => {
            if ty.is_sized(tcx.at(DUMMY_SP), ParamEnv::reveal_all()) {
                types::I64
            } else {
                return None;
            }
        }
        TypeVariants::TyParam(_) => bug!("{:?}: {:?}", ty, ty.sty),
        _ => return None,
    })
}

fn codegen_field<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx>,
    base: Value,
    layout: TyLayout<'tcx>,
    field: mir::Field,
) -> (Value, TyLayout<'tcx>) {
    let field_offset = layout.fields.offset(field.index());
    let field_ty = layout.field(&*fx, field.index());
    if field_offset.bytes() > 0 {
        let field_offset = fx.bcx.ins().iconst(types::I64, field_offset.bytes() as i64);
        (fx.bcx.ins().iadd(base, field_offset), field_ty)
    } else {
        (base, field_ty)
    }
}

/// A read-only value
#[derive(Debug, Copy, Clone)]
pub enum CValue<'tcx> {
    ByRef(Value, TyLayout<'tcx>),
    ByVal(Value, TyLayout<'tcx>),
    Func(FuncRef, TyLayout<'tcx>),
}

impl<'tcx> CValue<'tcx> {
    pub fn layout(&self) -> TyLayout<'tcx> {
        match *self {
            CValue::ByRef(_, layout) | CValue::ByVal(_, layout) | CValue::Func(_, layout) => layout,
        }
    }

    pub fn force_stack<'a>(self, fx: &mut FunctionCx<'a, 'tcx>) -> Value
    where
        'tcx: 'a,
    {
        match self {
            CValue::ByRef(value, _layout) => value,
            CValue::ByVal(value, layout) => {
                let stack_slot = fx.bcx.create_stack_slot(StackSlotData {
                    kind: StackSlotKind::ExplicitSlot,
                    size: layout.size.bytes() as u32,
                    offset: None,
                });
                fx.bcx.ins().stack_store(value, stack_slot, 0);
                fx.bcx.ins().stack_addr(types::I64, stack_slot, 0)
            }
            CValue::Func(func, ty) => {
                let func = fx.bcx.ins().func_addr(types::I64, func);
                CValue::ByVal(func, ty).force_stack(fx)
            }
        }
    }

    pub fn load_value<'a>(self, fx: &mut FunctionCx<'a, 'tcx>) -> Value
    where
        'tcx: 'a,
    {
        match self {
            CValue::ByRef(addr, layout) => {
                let cton_ty = fx
                    .cton_type(layout.ty)
                    .expect(&format!("load_value of type {:?}", layout.ty));
                fx.bcx.ins().load(cton_ty, MemFlags::new(), addr, 0)
            }
            CValue::ByVal(value, _layout) => value,
            CValue::Func(func, _layout) => fx.bcx.ins().func_addr(types::I64, func),
        }
    }

    pub fn expect_byref(self) -> (Value, TyLayout<'tcx>) {
        match self {
            CValue::ByRef(value, layout) => (value, layout),
            CValue::ByVal(_, _) => bug!("Expected CValue::ByRef, found CValue::ByVal: {:?}", self),
            CValue::Func(_, _) => bug!("Expected CValue::ByRef, found CValue::Func: {:?}", self),
        }
    }

    pub fn value_field<'a>(self, fx: &mut FunctionCx<'a, 'tcx>, field: mir::Field) -> CValue<'tcx>
    where
        'tcx: 'a,
    {
        let (base, layout) = match self {
            CValue::ByRef(addr, layout) => (addr, layout),
            _ => bug!("place_field for {:?}", self),
        };

        let (field_ptr, field_layout) = codegen_field(fx, base, layout, field);
        CValue::ByRef(field_ptr, field_layout)
    }

    pub fn const_val<'a>(
        fx: &mut FunctionCx<'a, 'tcx>,
        ty: Ty<'tcx>,
        const_val: i64,
    ) -> CValue<'tcx>
    where
        'tcx: 'a,
    {
        let cton_ty = fx.cton_type(ty).unwrap();
        let layout = fx.layout_of(ty);
        CValue::ByVal(fx.bcx.ins().iconst(cton_ty, const_val), layout)
    }

    pub fn unchecked_cast_to(self, layout: TyLayout<'tcx>) -> Self {
        match self {
            CValue::ByRef(addr, _) => CValue::ByRef(addr, layout),
            CValue::ByVal(val, _) => CValue::ByVal(val, layout),
            CValue::Func(fun, _) => CValue::Func(fun, layout),
        }
    }
}

/// A place where you can write a value to or read a value from
#[derive(Debug, Copy, Clone)]
pub enum CPlace<'tcx> {
    Var(Variable, TyLayout<'tcx>),
    Addr(Value, TyLayout<'tcx>),
}

impl<'a, 'tcx: 'a> CPlace<'tcx> {
    pub fn layout(&self) -> TyLayout<'tcx> {
        match *self {
            CPlace::Var(_, layout) | CPlace::Addr(_, layout) => layout,
        }
    }

    pub fn temp(fx: &mut FunctionCx<'a, 'tcx>, ty: Ty<'tcx>) -> CPlace<'tcx> {
        let layout = fx.layout_of(ty);
        let stack_slot = fx.bcx.create_stack_slot(StackSlotData {
            kind: StackSlotKind::ExplicitSlot,
            size: layout.size.bytes() as u32,
            offset: None,
        });
        CPlace::Addr(fx.bcx.ins().stack_addr(types::I64, stack_slot, 0), layout)
    }

    pub fn from_stack_slot(
        fx: &mut FunctionCx<'a, 'tcx>,
        stack_slot: StackSlot,
        ty: Ty<'tcx>,
    ) -> CPlace<'tcx> {
        let layout = fx.layout_of(ty);
        CPlace::Addr(fx.bcx.ins().stack_addr(types::I64, stack_slot, 0), layout)
    }

    pub fn to_cvalue(self, fx: &mut FunctionCx<'a, 'tcx>) -> CValue<'tcx> {
        match self {
            CPlace::Var(var, layout) => CValue::ByVal(fx.bcx.use_var(var), layout),
            CPlace::Addr(addr, layout) => CValue::ByRef(addr, layout),
        }
    }

    pub fn expect_addr(self) -> Value {
        match self {
            CPlace::Addr(addr, _layout) => addr,
            CPlace::Var(_, _) => bug!("Expected CPlace::Addr, found CPlace::Var"),
        }
    }

    pub fn write_cvalue(self, fx: &mut FunctionCx<'a, 'tcx>, from: CValue<'tcx>) {
        match (&self.layout().ty.sty, &from.layout().ty.sty) {
            (TypeVariants::TyRef(_, t, dest_mut), TypeVariants::TyRef(_, u, src_mut))
                if (if *dest_mut != ::rustc::hir::Mutability::MutImmutable && src_mut != dest_mut {
                    false
                } else if t != u {
                    false
                } else {
                    true
                }) =>
            {
                // &mut T -> &T is allowed
                // &'a T -> &'b T is allowed
            }
            _ => {
                assert_eq!(
                    self.layout().ty,
                    from.layout().ty,
                    "Can't write value of incompatible type to place {:?} {:?}\n\n{:#?}",
                    self.layout().ty.sty,
                    from.layout().ty.sty,
                    fx,
                );
            }
        }

        match self {
            CPlace::Var(var, _) => {
                let data = from.load_value(fx);
                fx.bcx.def_var(var, data)
            }
            CPlace::Addr(addr, layout) => {
                let size = layout.size.bytes() as i32;

                if let Some(_) = fx.cton_type(layout.ty) {
                    let data = from.load_value(fx);
                    fx.bcx.ins().store(MemFlags::new(), data, addr, 0);
                } else {
                    let from = from.expect_byref();
                    let mut offset = 0;
                    while size - offset >= 8 {
                        let byte = fx
                            .bcx
                            .ins()
                            .load(types::I64, MemFlags::new(), from.0, offset);
                        fx.bcx.ins().store(MemFlags::new(), byte, addr, offset);
                        offset += 8;
                    }
                    while size - offset >= 4 {
                        let byte = fx
                            .bcx
                            .ins()
                            .load(types::I32, MemFlags::new(), from.0, offset);
                        fx.bcx.ins().store(MemFlags::new(), byte, addr, offset);
                        offset += 4;
                    }
                    while offset < size {
                        let byte = fx
                            .bcx
                            .ins()
                            .load(types::I8, MemFlags::new(), from.0, offset);
                        fx.bcx.ins().store(MemFlags::new(), byte, addr, offset);
                        offset += 1;
                    }
                }
            }
        }
    }

    pub fn place_field(self, fx: &mut FunctionCx<'a, 'tcx>, field: mir::Field) -> CPlace<'tcx> {
        let base = self.expect_addr();
        let layout = self.layout();

        let (field_ptr, field_layout) = codegen_field(fx, base, layout, field);
        CPlace::Addr(field_ptr, field_layout)
    }

    pub fn place_index(self, fx: &mut FunctionCx<'a, 'tcx>, index: Value) -> CPlace<'tcx> {
        let addr = self.expect_addr();
        let layout = self.layout();
        match layout.ty.sty {
            TypeVariants::TyArray(elem_ty, _) => {
                let elem_layout = fx.layout_of(elem_ty);
                let size = fx
                    .bcx
                    .ins()
                    .iconst(types::I64, elem_layout.size.bytes() as i64);
                let offset = fx.bcx.ins().imul(size, index);
                CPlace::Addr(fx.bcx.ins().iadd(addr, offset), elem_layout)
            }
            TypeVariants::TySlice(_elem_ty) => unimplemented!("place_index(TySlice)"),
            _ => bug!("place_index({:?})", layout.ty),
        }
    }

    pub fn unchecked_cast_to(self, layout: TyLayout<'tcx>) -> Self {
        match self {
            CPlace::Var(var, _) => CPlace::Var(var, layout),
            CPlace::Addr(addr, _) => CPlace::Addr(addr, layout),
        }
    }

    pub fn downcast_variant(self, fx: &FunctionCx<'a, 'tcx>, variant: usize) -> Self {
        let layout = self.layout().for_variant(fx, variant);
        self.unchecked_cast_to(layout)
    }
}

pub fn cton_intcast<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx>,
    val: Value,
    from: Ty<'tcx>,
    to: Ty<'tcx>,
    signed: bool,
) -> Value {
    let from = fx.cton_type(from).unwrap();
    let to = fx.cton_type(to).unwrap();
    if from == to {
        return val;
    }
    if to.wider_or_equal(from) {
        if signed {
            fx.bcx.ins().sextend(to, val)
        } else {
            fx.bcx.ins().uextend(to, val)
        }
    } else {
        fx.bcx.ins().ireduce(to, val)
    }
}

pub struct FunctionCx<'a, 'tcx: 'a> {
    pub tcx: TyCtxt<'a, 'tcx, 'tcx>,
    pub module: &'a mut Module<CurrentBackend>,
    pub instance: Instance<'tcx>,
    pub mir: &'tcx Mir<'tcx>,
    pub param_substs: &'tcx Substs<'tcx>,
    pub bcx: FunctionBuilder<'a, Variable>,
    pub ebb_map: HashMap<BasicBlock, Ebb>,
    pub local_map: HashMap<Local, CPlace<'tcx>>,
    pub comments: HashMap<Inst, String>,
    pub constants: &'a mut HashMap<AllocId, DataId>,
}

impl<'a, 'tcx: 'a> fmt::Debug for FunctionCx<'a, 'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{:?}", self.param_substs)?;
        writeln!(f, "{:?}", self.local_map)?;

        let mut clif = String::new();
        let mut writer = crate::pretty_clif::CommentWriter(self.comments.clone());
        ::cranelift::codegen::write::decorate_function(
            &mut writer,
            &mut clif,
            &self.bcx.func,
            None,
        ).unwrap();
        writeln!(f, "\n{}", clif)
    }
}

impl<'a, 'tcx: 'a> LayoutOf for &'a FunctionCx<'a, 'tcx> {
    type Ty = Ty<'tcx>;
    type TyLayout = TyLayout<'tcx>;

    fn layout_of(self, ty: Ty<'tcx>) -> TyLayout<'tcx> {
        let ty = self.monomorphize(&ty);
        self.tcx.layout_of(ParamEnv::reveal_all().and(&ty)).unwrap()
    }
}

impl<'a, 'tcx> layout::HasTyCtxt<'tcx> for &'a FunctionCx<'a, 'tcx> {
    fn tcx<'b>(&'b self) -> TyCtxt<'b, 'tcx, 'tcx> {
        self.tcx
    }
}

impl<'a, 'tcx> layout::HasDataLayout for &'a FunctionCx<'a, 'tcx> {
    fn data_layout(&self) -> &layout::TargetDataLayout {
        &self.tcx.data_layout
    }
}

impl<'a, 'tcx> HasTargetSpec for &'a FunctionCx<'a, 'tcx> {
    fn target_spec(&self) -> &Target {
        &self.tcx.sess.target.target
    }
}

impl<'a, 'tcx: 'a> FunctionCx<'a, 'tcx> {
    pub fn monomorphize<T>(&self, value: &T) -> T
    where
        T: TypeFoldable<'tcx>,
    {
        self.tcx.subst_and_normalize_erasing_regions(
            self.param_substs,
            ty::ParamEnv::reveal_all(),
            value,
        )
    }

    pub fn cton_type(&self, ty: Ty<'tcx>) -> Option<Type> {
        cton_type_from_ty(self.tcx, self.monomorphize(&ty))
    }

    pub fn get_ebb(&self, bb: BasicBlock) -> Ebb {
        *self.ebb_map.get(&bb).unwrap()
    }

    pub fn get_local_place(&mut self, local: Local) -> CPlace<'tcx> {
        *self.local_map.get(&local).unwrap()
    }
}
