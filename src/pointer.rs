use crate::prelude::*;

use cranelift::codegen::ir::immediates::Offset32;

#[derive(Copy, Clone, Debug)]
pub struct Pointer {
    base: PointerBase,
    offset: Offset32,
}

#[derive(Copy, Clone, Debug)]
enum PointerBase {
    Addr(Value),
    Stack(StackSlot),
}

impl Pointer {
    pub fn new(addr: Value) -> Self {
        Pointer {
            base: PointerBase::Addr(addr),
            offset: Offset32::new(0),
        }
    }

    pub fn stack_slot(stack_slot: StackSlot) -> Self {
        Pointer {
            base: PointerBase::Stack(stack_slot),
            offset: Offset32::new(0),
        }
    }

    pub fn const_addr<'a, 'tcx>(fx: &mut FunctionCx<'a, 'tcx, impl Backend>, addr: i64) -> Self {
        let addr = fx.bcx.ins().iconst(fx.pointer_type, addr);
        Pointer {
            base: PointerBase::Addr(addr),
            offset: Offset32::new(0),
        }
    }

    pub fn get_addr<'a, 'tcx>(self, fx: &mut FunctionCx<'a, 'tcx, impl Backend>) -> Value {
        match self.base {
            PointerBase::Addr(base_addr) => {
                let offset: i64 = self.offset.into();
                if offset == 0 {
                    base_addr
                } else {
                    fx.bcx.ins().iadd_imm(base_addr, offset)
                }
            }
            PointerBase::Stack(stack_slot) => fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, self.offset),
        }
    }

    pub fn try_get_addr_and_offset(self) -> Option<(Value, Offset32)> {
        match self.base {
            PointerBase::Addr(addr) => Some((addr, self.offset)),
            PointerBase::Stack(_) => None,
        }
    }

    pub fn offset<'a, 'tcx>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        extra_offset: Offset32,
    ) -> Self {
        self.offset_i64(fx, extra_offset.into())
    }

    pub fn offset_i64<'a, 'tcx>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        extra_offset: i64,
    ) -> Self {
        if let Some(new_offset) = self.offset.try_add_i64(extra_offset) {
            Pointer {
                base: self.base,
                offset: new_offset,
            }
        } else {
            let base_offset: i64 = self.offset.into();
            if let Some(new_offset) = base_offset.checked_add(extra_offset){
                let base_addr = match self.base {
                    PointerBase::Addr(addr) => addr,
                    PointerBase::Stack(stack_slot) => fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0),
                };
                let addr = fx.bcx.ins().iadd_imm(base_addr, new_offset);
                Pointer {
                    base: PointerBase::Addr(addr),
                    offset: Offset32::new(0),
                }
            } else {
                panic!("self.offset ({}) + extra_offset ({}) not representable in i64", base_offset, extra_offset);
            }
        }
    }

    pub fn offset_value<'a, 'tcx>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        extra_offset: Value,
    ) -> Self {
        match self.base {
            PointerBase::Addr(addr) => Pointer {
                base: PointerBase::Addr(fx.bcx.ins().iadd(addr, extra_offset)),
                offset: self.offset,
            },
            PointerBase::Stack(stack_slot) => {
                let base_addr = fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, self.offset);
                Pointer {
                    base: PointerBase::Addr(fx.bcx.ins().iadd(base_addr, extra_offset)),
                    offset: Offset32::new(0),
                }
            }
        }
    }

    pub fn load<'a, 'tcx>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        ty: Type,
        flags: MemFlags,
    ) -> Value {
        match self.base {
            PointerBase::Addr(base_addr) => fx.bcx.ins().load(ty, flags, base_addr, self.offset),
            PointerBase::Stack(stack_slot) => if ty == types::I128 {
                // WORKAROUND for stack_load.i128 not being implemented
                let base_addr = fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0);
                fx.bcx.ins().load(ty, flags, base_addr, self.offset)
            } else {
                fx.bcx.ins().stack_load(ty, stack_slot, self.offset)
            }
        }
    }

    pub fn store<'a, 'tcx>(
        self,
        fx: &mut FunctionCx<'a, 'tcx, impl Backend>,
        value: Value,
        flags: MemFlags,
    ) {
        match self.base {
            PointerBase::Addr(base_addr) => {
                fx.bcx.ins().store(flags, value, base_addr, self.offset);
            }
            PointerBase::Stack(stack_slot) => if fx.bcx.func.dfg.value_type(value) == types::I128 {
                // WORKAROUND for stack_load.i128 not being implemented
                let base_addr = fx.bcx.ins().stack_addr(fx.pointer_type, stack_slot, 0);
                fx.bcx.ins().store(flags, value, base_addr, self.offset);
            } else {
                fx.bcx.ins().stack_store(value, stack_slot, self.offset);
            }
        }
    }
}
