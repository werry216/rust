#![allow(unknown_lints)]
#![allow(float_cmp)]

use std::mem::transmute;

use rustc::mir;

use error::{EvalError, EvalResult};
use memory::{AllocId, Pointer};

fn bits_to_f32(bits: u64) -> f32 {
    unsafe { transmute::<u32, f32>(bits as u32) }
}

fn bits_to_f64(bits: u64) -> f64 {
    unsafe { transmute::<u64, f64>(bits) }
}

fn f32_to_bits(f: f32) -> u64 {
    unsafe { transmute::<f32, u32>(f) as u64 }
}

fn f64_to_bits(f: f64) -> u64 {
    unsafe { transmute::<f64, u64>(f) }
}

fn bits_to_bool(n: u64) -> bool {
    // FIXME(solson): Can we reach here due to user error?
    debug_assert!(n == 0 || n == 1, "bits interpreted as bool were {}", n);
    n & 1 == 1
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimVal {
    pub bits: u64,

    /// This field is initialized when the `PrimVal` represents a pointer into an `Allocation`. An
    /// `Allocation` in the `memory` module has a list of relocations, but a `PrimVal` is only
    /// large enough to contain one, hence the `Option`.
    pub relocation: Option<AllocId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrimValKind {
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    F32, F64,
    Bool,
    Char,
    Ptr,
    FnPtr,
}

impl PrimValKind {
    pub fn is_int(self) -> bool {
        use self::PrimValKind::*;
        match self {
            I8 | I16 | I32 | I64 | U8 | U16 | U32 | U64 => true,
            _ => false,
        }
    }

    pub fn from_uint_size(size: u64) -> Self {
        match size {
            1 => PrimValKind::U8,
            2 => PrimValKind::U16,
            4 => PrimValKind::U32,
            8 => PrimValKind::U64,
            _ => bug!("can't make uint with size {}", size),
        }
    }

    pub fn from_int_size(size: u64) -> Self {
        match size {
            1 => PrimValKind::I8,
            2 => PrimValKind::I16,
            4 => PrimValKind::I32,
            8 => PrimValKind::I64,
            _ => bug!("can't make int with size {}", size),
        }
    }
}

impl PrimVal {
    pub fn new(bits: u64) -> Self {
        PrimVal { bits: bits, relocation: None }
    }

    pub fn new_with_relocation(bits: u64, alloc_id: AllocId) -> Self {
        PrimVal { bits: bits, relocation: Some(alloc_id) }
    }

    pub fn from_ptr(ptr: Pointer) -> Self {
        PrimVal::new_with_relocation(ptr.offset as u64, ptr.alloc_id)
    }

    pub fn from_bool(b: bool) -> Self {
        PrimVal::new(b as u64)
    }

    pub fn from_char(c: char) -> Self {
        PrimVal::new(c as u64)
    }

    pub fn from_f32(f: f32) -> Self {
        PrimVal::new(f32_to_bits(f))
    }

    pub fn from_f64(f: f64) -> Self {
        PrimVal::new(f64_to_bits(f))
    }

    pub fn from_uint(n: u64) -> Self {
        PrimVal::new(n)
    }

    pub fn from_int(n: i64) -> Self {
        PrimVal::new(n as u64)
    }

    pub fn to_f32(self) -> f32 {
        assert!(self.relocation.is_none());
        bits_to_f32(self.bits)
    }

    pub fn to_f64(self) -> f64 {
        assert!(self.relocation.is_none());
        bits_to_f64(self.bits)
    }

    pub fn to_ptr(self) -> Pointer {
        self.relocation.map(|alloc_id| {
            Pointer::new(alloc_id, self.bits)
        }).unwrap_or_else(|| Pointer::from_int(self.bits))
    }

    pub fn try_as_uint<'tcx>(self) -> EvalResult<'tcx, u64> {
        self.to_ptr().to_int().map(|val| val)
    }

    pub fn to_u64(self) -> u64 {
        if let Some(ptr) = self.try_as_ptr() {
            return ptr.to_int().expect("non abstract ptr") as u64;
        }
        self.bits
    }

    pub fn to_i64(self) -> i64 {
        if let Some(ptr) = self.try_as_ptr() {
            return ptr.to_int().expect("non abstract ptr") as i64;
        }
        self.bits as i64
    }

    pub fn try_as_ptr(self) -> Option<Pointer> {
        self.relocation.map(|alloc_id| {
            Pointer::new(alloc_id, self.bits)
        })
    }

    pub fn try_as_bool<'tcx>(self) -> EvalResult<'tcx, bool> {
        match self.bits {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(EvalError::InvalidBool),
        }
    }

}

////////////////////////////////////////////////////////////////////////////////
// MIR operator evaluation
////////////////////////////////////////////////////////////////////////////////

macro_rules! overflow {
    ($op:ident, $l:expr, $r:expr) => ({
        let (val, overflowed) = $l.$op($r);
        let primval = PrimVal::new(val as u64);
        Ok((primval, overflowed))
    })
}

macro_rules! int_arithmetic {
    ($kind:expr, $int_op:ident, $l:expr, $r:expr) => ({
        let l = $l;
        let r = $r;
        match $kind {
            I8  => overflow!($int_op, l as i8,  r as i8),
            I16 => overflow!($int_op, l as i16, r as i16),
            I32 => overflow!($int_op, l as i32, r as i32),
            I64 => overflow!($int_op, l as i64, r as i64),
            U8  => overflow!($int_op, l as u8,  r as u8),
            U16 => overflow!($int_op, l as u16, r as u16),
            U32 => overflow!($int_op, l as u32, r as u32),
            U64 => overflow!($int_op, l as u64, r as u64),
            _ => bug!("int_arithmetic should only be called on int primvals"),
        }
    })
}

macro_rules! int_shift {
    ($kind:expr, $int_op:ident, $l:expr, $r:expr) => ({
        let l = $l;
        let r = $r;
        match $kind {
            I8  => overflow!($int_op, l as i8,  r),
            I16 => overflow!($int_op, l as i16, r),
            I32 => overflow!($int_op, l as i32, r),
            I64 => overflow!($int_op, l as i64, r),
            U8  => overflow!($int_op, l as u8,  r),
            U16 => overflow!($int_op, l as u16, r),
            U32 => overflow!($int_op, l as u32, r),
            U64 => overflow!($int_op, l as u64, r),
            _ => bug!("int_shift should only be called on int primvals"),
        }
    })
}

macro_rules! float_arithmetic {
    ($from_bits:ident, $to_bits:ident, $float_op:tt, $l:expr, $r:expr) => ({
        let l = $from_bits($l);
        let r = $from_bits($r);
        let bits = $to_bits(l $float_op r);
        PrimVal::new(bits)
    })
}

macro_rules! f32_arithmetic {
    ($float_op:tt, $l:expr, $r:expr) => (
        float_arithmetic!(bits_to_f32, f32_to_bits, $float_op, $l, $r)
    )
}

macro_rules! f64_arithmetic {
    ($float_op:tt, $l:expr, $r:expr) => (
        float_arithmetic!(bits_to_f64, f64_to_bits, $float_op, $l, $r)
    )
}

/// Returns the result of the specified operation and whether it overflowed.
pub fn binary_op<'tcx>(
    bin_op: mir::BinOp,
    left: PrimVal,
    left_kind: PrimValKind,
    right: PrimVal,
    right_kind: PrimValKind,
) -> EvalResult<'tcx, (PrimVal, bool)> {
    use rustc::mir::BinOp::*;
    use self::PrimValKind::*;

    // If the pointers are into the same allocation, fall through to the more general match
    // later, which will do comparisons on the `bits` fields, which are the pointer offsets
    // in this case.
    let left_ptr = left.to_ptr();
    let right_ptr = right.to_ptr();
    if left_ptr.alloc_id != right_ptr.alloc_id {
        return Ok((unrelated_ptr_ops(bin_op, left_ptr, right_ptr)?, false));
    }

    let (l, r) = (left.bits, right.bits);

    // These ops can have an RHS with a different numeric type.
    if bin_op == Shl || bin_op == Shr {
        // These are the maximum values a bitshift RHS could possibly have. For example, u16
        // can be bitshifted by 0..16, so masking with 0b1111 (16 - 1) will ensure we are in
        // that range.
        let type_bits: u32 = match left_kind {
            I8  | U8  => 8,
            I16 | U16 => 16,
            I32 | U32 => 32,
            I64 | U64 => 64,
            _ => bug!("bad MIR: bitshift lhs is not integral"),
        };

        // Cast to `u32` because `overflowing_sh{l,r}` only take `u32`, then apply the bitmask
        // to ensure it's within the valid shift value range.
        let r = (right.bits as u32) & (type_bits - 1);

        return match bin_op {
            Shl => int_shift!(left_kind, overflowing_shl, l, r),
            Shr => int_shift!(left_kind, overflowing_shr, l, r),
            _ => bug!("it has already been checked that this is a shift op"),
        };
    }

    if left_kind != right_kind {
        let msg = format!("unimplemented binary op: {:?}, {:?}, {:?}", left, right, bin_op);
        return Err(EvalError::Unimplemented(msg));
    }

    let val = match (bin_op, left_kind) {
        (Eq, F32) => PrimVal::from_bool(bits_to_f32(l) == bits_to_f32(r)),
        (Ne, F32) => PrimVal::from_bool(bits_to_f32(l) != bits_to_f32(r)),
        (Lt, F32) => PrimVal::from_bool(bits_to_f32(l) <  bits_to_f32(r)),
        (Le, F32) => PrimVal::from_bool(bits_to_f32(l) <= bits_to_f32(r)),
        (Gt, F32) => PrimVal::from_bool(bits_to_f32(l) >  bits_to_f32(r)),
        (Ge, F32) => PrimVal::from_bool(bits_to_f32(l) >= bits_to_f32(r)),

        (Eq, F64) => PrimVal::from_bool(bits_to_f64(l) == bits_to_f64(r)),
        (Ne, F64) => PrimVal::from_bool(bits_to_f64(l) != bits_to_f64(r)),
        (Lt, F64) => PrimVal::from_bool(bits_to_f64(l) <  bits_to_f64(r)),
        (Le, F64) => PrimVal::from_bool(bits_to_f64(l) <= bits_to_f64(r)),
        (Gt, F64) => PrimVal::from_bool(bits_to_f64(l) >  bits_to_f64(r)),
        (Ge, F64) => PrimVal::from_bool(bits_to_f64(l) >= bits_to_f64(r)),

        (Add, F32) => f32_arithmetic!(+, l, r),
        (Sub, F32) => f32_arithmetic!(-, l, r),
        (Mul, F32) => f32_arithmetic!(*, l, r),
        (Div, F32) => f32_arithmetic!(/, l, r),
        (Rem, F32) => f32_arithmetic!(%, l, r),

        (Add, F64) => f64_arithmetic!(+, l, r),
        (Sub, F64) => f64_arithmetic!(-, l, r),
        (Mul, F64) => f64_arithmetic!(*, l, r),
        (Div, F64) => f64_arithmetic!(/, l, r),
        (Rem, F64) => f64_arithmetic!(%, l, r),

        (Eq, _) => PrimVal::from_bool(l == r),
        (Ne, _) => PrimVal::from_bool(l != r),
        (Lt, _) => PrimVal::from_bool(l <  r),
        (Le, _) => PrimVal::from_bool(l <= r),
        (Gt, _) => PrimVal::from_bool(l >  r),
        (Ge, _) => PrimVal::from_bool(l >= r),

        (BitOr,  _) => PrimVal::new(l | r),
        (BitAnd, _) => PrimVal::new(l & r),
        (BitXor, _) => PrimVal::new(l ^ r),

        (Add, k) if k.is_int() => return int_arithmetic!(k, overflowing_add, l, r),
        (Sub, k) if k.is_int() => return int_arithmetic!(k, overflowing_sub, l, r),
        (Mul, k) if k.is_int() => return int_arithmetic!(k, overflowing_mul, l, r),
        (Div, k) if k.is_int() => return int_arithmetic!(k, overflowing_div, l, r),
        (Rem, k) if k.is_int() => return int_arithmetic!(k, overflowing_rem, l, r),

        _ => {
            let msg = format!("unimplemented binary op: {:?}, {:?}, {:?}", left, right, bin_op);
            return Err(EvalError::Unimplemented(msg));
        }
    };

    Ok((val, false))
}

fn unrelated_ptr_ops<'tcx>(bin_op: mir::BinOp, left: Pointer, right: Pointer) -> EvalResult<'tcx, PrimVal> {
    use rustc::mir::BinOp::*;
    match bin_op {
        Eq => Ok(PrimVal::from_bool(false)),
        Ne => Ok(PrimVal::from_bool(true)),
        Lt | Le | Gt | Ge => Err(EvalError::InvalidPointerMath),
        _ if left.to_int().is_ok() ^ right.to_int().is_ok() => {
            Err(EvalError::ReadPointerAsBytes)
        },
        _ => bug!(),
    }
}

pub fn unary_op<'tcx>(
    un_op: mir::UnOp,
    val: PrimVal,
    val_kind: PrimValKind,
) -> EvalResult<'tcx, PrimVal> {
    use rustc::mir::UnOp::*;
    use self::PrimValKind::*;

    let bits = match (un_op, val_kind) {
        (Not, Bool) => !bits_to_bool(val.bits) as u64,

        (Not, U8)  => !(val.bits as u8) as u64,
        (Not, U16) => !(val.bits as u16) as u64,
        (Not, U32) => !(val.bits as u32) as u64,
        (Not, U64) => !val.bits,

        (Not, I8)  => !(val.bits as i8) as u64,
        (Not, I16) => !(val.bits as i16) as u64,
        (Not, I32) => !(val.bits as i32) as u64,
        (Not, I64) => !(val.bits as i64) as u64,

        (Neg, I8)  => -(val.bits as i8) as u64,
        (Neg, I16) => -(val.bits as i16) as u64,
        (Neg, I32) => -(val.bits as i32) as u64,
        (Neg, I64) => -(val.bits as i64) as u64,

        (Neg, F32) => f32_to_bits(-bits_to_f32(val.bits)),
        (Neg, F64) => f64_to_bits(-bits_to_f64(val.bits)),

        _ => {
            let msg = format!("unimplemented unary op: {:?}, {:?}", un_op, val);
            return Err(EvalError::Unimplemented(msg));
        }
    };

    Ok(PrimVal::new(bits))
}
