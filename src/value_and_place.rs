use crate::prelude::*;

fn codegen_field<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    base: Value,
    layout: TyLayout<'tcx>,
    field: mir::Field,
) -> (Value, TyLayout<'tcx>) {
    let field_offset = layout.fields.offset(field.index());
    let field_ty = layout.field(&*fx, field.index());
    if field_offset.bytes() > 0 {
        (
            fx.bcx.ins().iadd_imm(base, field_offset.bytes() as i64),
            field_ty,
        )
    } else {
        (base, field_ty)
    }
}

fn scalar_pair_calculate_b_offset(tcx: TyCtxt<'_>, a_scalar: &Scalar, b_scalar: &Scalar) -> i32 {
    let b_offset = a_scalar.value.size(&tcx).align_to(b_scalar.value.align(&tcx).abi);
    b_offset.bytes().try_into().unwrap()
}

/// A read-only value
#[derive(Debug, Copy, Clone)]
pub struct CValue<'tcx>(CValueInner, TyLayout<'tcx>);

#[derive(Debug, Copy, Clone)]
enum CValueInner {
    ByRef(Value),
    ByVal(Value),
    ByValPair(Value, Value),
}

fn store_scalar<'a, 'tcx: 'a>(fx: &mut FunctionCx<'a, 'tcx, impl Backend>, value: Value, addr: Value, offset: i32) {
    if fx.bcx.func.dfg.value_type(value) == types::I128 {
        let (a, b) = fx.bcx.ins().isplit(value);
        fx.bcx.ins().store(MemFlags::new(), a, addr, offset);
        fx.bcx.ins().store(MemFlags::new(), b, addr, offset + 8);
    } else {
        fx.bcx.ins().store(MemFlags::new(), value, addr, offset);
    }
}

fn load_scalar<'a, 'tcx: 'a>(
    fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    clif_ty: Type,
    addr: Value,
    offset: i32,
) -> Value {
    if clif_ty == types::I128 {
        let a = fx.bcx.ins().load(clif_ty, MemFlags::new(), addr, offset);
        let b = fx.bcx.ins().load(clif_ty, MemFlags::new(), addr, offset + 8);
        fx.bcx.ins().iconcat(a, b)
    } else {
        fx.bcx.ins().load(clif_ty, MemFlags::new(), addr, offset)
    }
}

impl<'tcx> CValue<'tcx> {
    pub fn by_ref(value: Value, layout: TyLayout<'tcx>) -> CValue<'tcx> {
        CValue(CValueInner::ByRef(value), layout)
    }

    pub fn by_val(value: Value, layout: TyLayout<'tcx>) -> CValue<'tcx> {
        CValue(CValueInner::ByVal(value), layout)
    }

    pub fn by_val_pair(value: Value, extra: Value, layout: TyLayout<'tcx>) -> CValue<'tcx> {
        CValue(CValueInner::ByValPair(value, extra), layout)
    }

    pub fn layout(&self) -> TyLayout<'tcx> {
        self.1
    }

    pub fn force_stack<'a>(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> Value
    where
        'tcx: 'a,
    {
        let layout = self.1;
        match self.0 {
            CValueInner::ByRef(value) => value,
            CValueInner::ByVal(_) | CValueInner::ByValPair(_, _) => {
                let cplace = CPlace::new_stack_slot(fx, layout.ty);
                cplace.write_cvalue(fx, self);
                cplace.to_addr(fx)
            }
        }
    }

    /// Load a value with layout.abi of scalar
    pub fn load_scalar<'a>(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> Value
    where
        'tcx: 'a,
    {
        let layout = self.1;
        match self.0 {
            CValueInner::ByRef(addr) => {
                let scalar = match layout.abi {
                    layout::Abi::Scalar(ref scalar) => scalar.clone(),
                    _ => unreachable!(),
                };
                let clif_ty = scalar_to_clif_type(fx.tcx, scalar);
                load_scalar(fx, clif_ty, addr, 0)
            }
            CValueInner::ByVal(value) => value,
            CValueInner::ByValPair(_, _) => bug!("Please use load_scalar_pair for ByValPair"),
        }
    }

    /// Load a value pair with layout.abi of scalar pair
    pub fn load_scalar_pair<'a>(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> (Value, Value)
    where
        'tcx: 'a,
    {
        let layout = self.1;
        match self.0 {
            CValueInner::ByRef(addr) => {
                let (a_scalar, b_scalar) = match &layout.abi {
                    layout::Abi::ScalarPair(a, b) => (a, b),
                    _ => unreachable!(),
                };
                let b_offset = scalar_pair_calculate_b_offset(fx.tcx, a_scalar, b_scalar);
                let clif_ty1 = scalar_to_clif_type(fx.tcx, a_scalar.clone());
                let clif_ty2 = scalar_to_clif_type(fx.tcx, b_scalar.clone());
                let val1 = load_scalar(fx, clif_ty1, addr, 0);
                let val2 = load_scalar(
                    fx,
                    clif_ty2,
                    addr,
                    b_offset,
                );
                (val1, val2)
            }
            CValueInner::ByVal(_) => bug!("Please use load_scalar for ByVal"),
            CValueInner::ByValPair(val1, val2) => (val1, val2),
        }
    }

    pub fn value_field<'a>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        field: mir::Field,
    ) -> CValue<'tcx>
    where
        'tcx: 'a,
    {
        let layout = self.1;
        let base = match self.0 {
            CValueInner::ByRef(addr) => addr,
            _ => bug!("place_field for {:?}", self),
        };

        let (field_ptr, field_layout) = codegen_field(fx, base, layout, field);
        CValue::by_ref(field_ptr, field_layout)
    }

    pub fn unsize_value<'a>(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>, dest: CPlace<'tcx>) {
        crate::unsize::coerce_unsized_into(fx, self, dest);
    }

    pub fn const_val<'a>(
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        ty: Ty<'tcx>,
        const_val: i64,
    ) -> CValue<'tcx>
    where
        'tcx: 'a,
    {
        let clif_ty = fx.clif_type(ty).unwrap();
        let layout = fx.layout_of(ty);
        let val = if clif_ty == types::I128 {
            // FIXME don't assume little-endian arch
            let lsb = fx.bcx.ins().iconst(types::I64, const_val);
            let msb = fx.bcx.ins().iconst(types::I64, 0);
            fx.bcx.ins().iconcat(lsb, msb)
        } else {
            fx.bcx.ins().iconst(clif_ty, const_val)
        };
        CValue::by_val(val, layout)
    }

    pub fn unchecked_cast_to(self, layout: TyLayout<'tcx>) -> Self {
        CValue(self.0, layout)
    }
}

/// A place where you can write a value to or read a value from
#[derive(Debug, Copy, Clone)]
pub enum CPlace<'tcx> {
    Var(Local, TyLayout<'tcx>),
    Addr(Value, Option<Value>, TyLayout<'tcx>),
    Stack(StackSlot, TyLayout<'tcx>),
    NoPlace(TyLayout<'tcx>),
}

impl<'a, 'tcx: 'a> CPlace<'tcx> {
    pub fn layout(&self) -> TyLayout<'tcx> {
        match *self {
            CPlace::Var(_, layout)
            | CPlace::Addr(_, _, layout)
            | CPlace::Stack(_, layout)
            | CPlace::NoPlace(layout) => layout,
        }
    }

    pub fn no_place(layout: TyLayout<'tcx>) -> CPlace<'tcx> {
        CPlace::NoPlace(layout)
    }

    pub fn new_stack_slot(
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        ty: Ty<'tcx>,
    ) -> CPlace<'tcx> {
        let layout = fx.layout_of(ty);
        assert!(!layout.is_unsized());
        if layout.size.bytes() == 0 {
            return CPlace::NoPlace(layout);
        }

        let stack_slot = fx.bcx.create_stack_slot(StackSlotData {
            kind: StackSlotKind::ExplicitSlot,
            size: layout.size.bytes() as u32,
            offset: None,
        });
        CPlace::Stack(stack_slot, layout)
    }

    pub fn new_var(
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        local: Local,
        layout: TyLayout<'tcx>,
    ) -> CPlace<'tcx> {
        fx.bcx
            .declare_var(mir_var(local), fx.clif_type(layout.ty).unwrap());
        CPlace::Var(local, layout)
    }

    pub fn for_addr(addr: Value, layout: TyLayout<'tcx>) -> CPlace<'tcx> {
        CPlace::Addr(addr, None, layout)
    }

    pub fn for_addr_with_extra(addr: Value, extra: Value, layout: TyLayout<'tcx>) -> CPlace<'tcx> {
        CPlace::Addr(addr, Some(extra), layout)
    }

    pub fn to_cvalue(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> CValue<'tcx> {
        match self {
            CPlace::Var(var, layout) => CValue::by_val(fx.bcx.use_var(mir_var(var)), layout),
            CPlace::Addr(addr, extra, layout) => {
                assert!(extra.is_none(), "unsized values are not yet supported");
                CValue::by_ref(addr, layout)
            }
            CPlace::Stack(stack_slot, layout) => CValue::by_ref(
                fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0),
                layout,
            ),
            CPlace::NoPlace(layout) => CValue::by_ref(
                fx.bcx
                    .ins()
                    .iconst(fx.pointer_type, fx.pointer_type.bytes() as i64),
                layout,
            ),
        }
    }

    pub fn to_addr(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> Value {
        match self.to_addr_maybe_unsized(fx) {
            (addr, None) => addr,
            (_, Some(_)) => bug!("Expected sized cplace, found {:?}", self),
        }
    }

    pub fn to_addr_maybe_unsized(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
    ) -> (Value, Option<Value>) {
        match self {
            CPlace::Addr(addr, extra, _layout) => (addr, extra),
            CPlace::Stack(stack_slot, _layout) => (
                fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0),
                None,
            ),
            CPlace::NoPlace(_) => (fx.bcx.ins().iconst(fx.pointer_type, 45), None),
            CPlace::Var(_, _) => bug!("Expected CPlace::Addr, found CPlace::Var"),
        }
    }

    pub fn write_cvalue(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>, from: CValue<'tcx>) {
        use rustc::hir::Mutability::*;

        let from_ty = from.layout().ty;
        let to_ty = self.layout().ty;

        fn assert_assignable<'a, 'tcx: 'a>(fx: &FunctionCx<'a, 'tcx, impl Backend>, from_ty: Ty<'tcx>, to_ty: Ty<'tcx>) {
            match (&from_ty.sty, &to_ty.sty) {
                (ty::Ref(_, t, MutImmutable), ty::Ref(_, u, MutImmutable))
                | (ty::Ref(_, t, MutMutable), ty::Ref(_, u, MutImmutable))
                | (ty::Ref(_, t, MutMutable), ty::Ref(_, u, MutMutable)) => {
                    assert_assignable(fx, t, u);
                    // &mut T -> &T is allowed
                    // &'a T -> &'b T is allowed
                }
                (ty::Ref(_, _, MutImmutable), ty::Ref(_, _, MutMutable)) => {
                    panic!("Cant assign value of type {} to place of type {}", from_ty, to_ty)
                }
                (ty::FnPtr(_), ty::FnPtr(_)) => {
                    let from_sig = fx.tcx.normalize_erasing_late_bound_regions(
                        ParamEnv::reveal_all(),
                        &from_ty.fn_sig(fx.tcx),
                    );
                    let to_sig = fx.tcx.normalize_erasing_late_bound_regions(
                        ParamEnv::reveal_all(),
                        &to_ty.fn_sig(fx.tcx),
                    );
                    assert_eq!(
                        from_sig, to_sig,
                        "Can't write fn ptr with incompatible sig {:?} to place with sig {:?}\n\n{:#?}",
                        from_sig, to_sig, fx,
                    );
                    // fn(&T) -> for<'l> fn(&'l T) is allowed
                }
                (ty::Dynamic(from_traits, _), ty::Dynamic(to_traits, _)) => {
                    let from_traits = fx.tcx.normalize_erasing_late_bound_regions(
                        ParamEnv::reveal_all(),
                        from_traits,
                    );
                    let to_traits = fx.tcx.normalize_erasing_late_bound_regions(
                        ParamEnv::reveal_all(),
                        to_traits,
                    );
                    assert_eq!(
                        from_traits, to_traits,
                        "Can't write trait object of incompatible traits {:?} to place with traits {:?}\n\n{:#?}",
                        from_traits, to_traits, fx,
                    );
                    // dyn for<'r> Trait<'r> -> dyn Trait<'_> is allowed
                }
                _ => {
                    assert_eq!(
                        from_ty,
                        to_ty,
                        "Can't write value with incompatible type {:?} to place with type {:?}\n\n{:#?}",
                        from_ty,
                        to_ty,
                        fx,
                    );
                }
            }
        }

        assert_assignable(fx, from_ty, to_ty);

        let (addr, dst_layout) = match self {
            CPlace::Var(var, _) => {
                let data = from.load_scalar(fx);
                fx.bcx.def_var(mir_var(var), data);
                return;
            }
            CPlace::Addr(addr, None, dst_layout) => (addr, dst_layout),
            CPlace::Stack(stack_slot, dst_layout) => (
                fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0),
                dst_layout,
            ),
            CPlace::NoPlace(layout) => {
                if layout.abi != Abi::Uninhabited {
                    assert_eq!(layout.size.bytes(), 0, "{:?}", layout);
                }
                return;
            }
            CPlace::Addr(_, _, _) => bug!("Can't write value to unsized place {:?}", self),
        };

        match from.0 {
            CValueInner::ByVal(val) => {
                store_scalar(fx, val, addr, 0);
            }
            CValueInner::ByValPair(value, extra) => {
                match dst_layout.abi {
                    Abi::ScalarPair(ref a_scalar, ref b_scalar) => {
                        let b_offset = scalar_pair_calculate_b_offset(fx.tcx, a_scalar, b_scalar);
                        store_scalar(fx, value, addr, 0);
                        store_scalar(
                            fx,
                            extra,
                            addr,
                            b_offset,
                        );
                    }
                    _ => bug!(
                        "Non ScalarPair abi {:?} for ByValPair CValue",
                        dst_layout.abi
                    ),
                }
            }
            CValueInner::ByRef(from_addr) => {
                let src_layout = from.1;
                let size = dst_layout.size.bytes();
                let src_align = src_layout.align.abi.bytes() as u8;
                let dst_align = dst_layout.align.abi.bytes() as u8;
                fx.bcx.emit_small_memcpy(
                    fx.module.target_config(),
                    addr,
                    from_addr,
                    size,
                    dst_align,
                    src_align,
                );
            }
        }
    }

    pub fn place_field(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        field: mir::Field,
    ) -> CPlace<'tcx> {
        let layout = self.layout();
        let (base, extra) = self.to_addr_maybe_unsized(fx);

        let (field_ptr, field_layout) = codegen_field(fx, base, layout, field);
        let extra = if field_layout.is_unsized() {
            assert!(extra.is_some());
            extra
        } else {
            None
        };
        CPlace::Addr(field_ptr, extra, field_layout)
    }

    pub fn place_index(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        index: Value,
    ) -> CPlace<'tcx> {
        let (elem_layout, addr) = match self.layout().ty.sty {
            ty::Array(elem_ty, _) => (fx.layout_of(elem_ty), self.to_addr(fx)),
            ty::Slice(elem_ty) => (fx.layout_of(elem_ty), self.to_addr_maybe_unsized(fx).0),
            _ => bug!("place_index({:?})", self.layout().ty),
        };

        let offset = fx
            .bcx
            .ins()
            .imul_imm(index, elem_layout.size.bytes() as i64);

        CPlace::Addr(fx.bcx.ins().iadd(addr, offset), None, elem_layout)
    }

    pub fn place_deref(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> CPlace<'tcx> {
        let inner_layout = fx.layout_of(self.layout().ty.builtin_deref(true).unwrap().ty);
        if !inner_layout.is_unsized() {
            CPlace::Addr(self.to_cvalue(fx).load_scalar(fx), None, inner_layout)
        } else {
            let (addr, extra) = self.to_cvalue(fx).load_scalar_pair(fx);
            CPlace::Addr(addr, Some(extra), inner_layout)
        }
    }

    pub fn write_place_ref(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>, dest: CPlace<'tcx>) {
        if !self.layout().is_unsized() {
            let ptr = CValue::by_val(self.to_addr(fx), dest.layout());
            dest.write_cvalue(fx, ptr);
        } else {
            let (value, extra) = self.to_addr_maybe_unsized(fx);
            let ptr = CValue::by_val_pair(value, extra.expect("unsized type without metadata"), dest.layout());
            dest.write_cvalue(fx, ptr);
        }
    }

    pub fn unchecked_cast_to(self, layout: TyLayout<'tcx>) -> Self {
        assert!(!self.layout().is_unsized());
        match self {
            CPlace::Var(var, _) => CPlace::Var(var, layout),
            CPlace::Addr(addr, extra, _) => CPlace::Addr(addr, extra, layout),
            CPlace::Stack(stack_slot, _) => CPlace::Stack(stack_slot, layout),
            CPlace::NoPlace(_) => {
                assert!(layout.size.bytes() == 0);
                CPlace::NoPlace(layout)
            }
        }
    }

    pub fn downcast_variant(
        self,
        fx: &FunctionCx<'a, 'tcx, impl Backend>,
        variant: VariantIdx,
    ) -> Self {
        let layout = self.layout().for_variant(fx, variant);
        self.unchecked_cast_to(layout)
    }
}
