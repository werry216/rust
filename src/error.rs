use std::error::Error;
use std::fmt;
use rustc::mir;
use rustc::ty::{FnSig, Ty, layout};
use memory::Pointer;
use rustc_const_math::ConstMathErr;
use syntax::codemap::Span;

#[derive(Clone, Debug)]
pub enum EvalError<'tcx> {
    FunctionPointerTyMismatch(FnSig<'tcx>, FnSig<'tcx>),
    NoMirFor(String),
    UnterminatedCString(Pointer),
    DanglingPointerDeref,
    InvalidMemoryAccess,
    InvalidFunctionPointer,
    InvalidBool,
    InvalidDiscriminant,
    PointerOutOfBounds {
        ptr: Pointer,
        access: bool,
        allocation_size: u64,
    },
    ReadPointerAsBytes,
    ReadBytesAsPointer,
    InvalidPointerMath,
    ReadUndefBytes,
    DeadLocal,
    InvalidBoolOp(mir::BinOp),
    Unimplemented(String),
    DerefFunctionPointer,
    ExecuteMemory,
    ArrayIndexOutOfBounds(Span, u64, u64),
    Math(Span, ConstMathErr),
    OverflowingMath,
    InvalidChar(u128),
    OutOfMemory {
        allocation_size: u64,
        memory_size: u64,
        memory_usage: u64,
    },
    ExecutionTimeLimitReached,
    StackFrameLimitReached,
    OutOfTls,
    TlsOutOfBounds,
    AbiViolation(String),
    AlignmentCheckFailed {
        required: u64,
        has: u64,
    },
    CalledClosureAsFunction,
    VtableForArgumentlessMethod,
    ModifiedConstantMemory,
    AssumptionNotHeld,
    InlineAsm,
    TypeNotPrimitive(Ty<'tcx>),
    ReallocatedStaticMemory,
    DeallocatedStaticMemory,
    Layout(layout::LayoutError<'tcx>),
    Unreachable,
    Panic,
}

pub type EvalResult<'tcx, T = ()> = Result<T, EvalError<'tcx>>;

impl<'tcx> Error for EvalError<'tcx> {
    fn description(&self) -> &str {
        match *self {
            EvalError::FunctionPointerTyMismatch(..) =>
                "tried to call a function through a function pointer of a different type",
            EvalError::InvalidMemoryAccess =>
                "tried to access memory through an invalid pointer",
            EvalError::DanglingPointerDeref =>
                "dangling pointer was dereferenced",
            EvalError::InvalidFunctionPointer =>
                "tried to use an integer pointer or a dangling pointer as a function pointer",
            EvalError::InvalidBool =>
                "invalid boolean value read",
            EvalError::InvalidDiscriminant =>
                "invalid enum discriminant value read",
            EvalError::PointerOutOfBounds { .. } =>
                "pointer offset outside bounds of allocation",
            EvalError::ReadPointerAsBytes =>
                "a raw memory access tried to access part of a pointer value as raw bytes",
            EvalError::ReadBytesAsPointer =>
                "a memory access tried to interpret some bytes as a pointer",
            EvalError::InvalidPointerMath =>
                "attempted to do math or a comparison on pointers into different allocations",
            EvalError::ReadUndefBytes =>
                "attempted to read undefined bytes",
            EvalError::DeadLocal =>
                "tried to access a dead local variable",
            EvalError::InvalidBoolOp(_) =>
                "invalid boolean operation",
            EvalError::Unimplemented(ref msg) => msg,
            EvalError::DerefFunctionPointer =>
                "tried to dereference a function pointer",
            EvalError::ExecuteMemory =>
                "tried to treat a memory pointer as a function pointer",
            EvalError::ArrayIndexOutOfBounds(..) =>
                "array index out of bounds",
            EvalError::Math(..) =>
                "mathematical operation failed",
            EvalError::OverflowingMath =>
                "attempted to do overflowing math",
            EvalError::NoMirFor(..) =>
                "mir not found",
            EvalError::InvalidChar(..) =>
                "tried to interpret an invalid 32-bit value as a char",
            EvalError::OutOfMemory{..} =>
                "could not allocate more memory",
            EvalError::ExecutionTimeLimitReached =>
                "reached the configured maximum execution time",
            EvalError::StackFrameLimitReached =>
                "reached the configured maximum number of stack frames",
            EvalError::OutOfTls =>
                "reached the maximum number of representable TLS keys",
            EvalError::TlsOutOfBounds =>
                "accessed an invalid (unallocated) TLS key",
            EvalError::AbiViolation(ref msg) => msg,
            EvalError::AlignmentCheckFailed{..} =>
                "tried to execute a misaligned read or write",
            EvalError::CalledClosureAsFunction =>
                "tried to call a closure through a function pointer",
            EvalError::VtableForArgumentlessMethod =>
                "tried to call a vtable function without arguments",
            EvalError::ModifiedConstantMemory =>
                "tried to modify constant memory",
            EvalError::AssumptionNotHeld =>
                "`assume` argument was false",
            EvalError::InlineAsm =>
                "miri does not support inline assembly",
            EvalError::TypeNotPrimitive(_) =>
                "expected primitive type, got nonprimitive",
            EvalError::ReallocatedStaticMemory =>
                "tried to reallocate static memory",
            EvalError::DeallocatedStaticMemory =>
                "tried to deallocate static memory",
            EvalError::Layout(_) =>
                "rustc layout computation failed",
            EvalError::UnterminatedCString(_) =>
                "attempted to get length of a null terminated string, but no null found before end of allocation",
            EvalError::Unreachable =>
                "entered unreachable code",
            EvalError::Panic =>
                "the evaluated program panicked",
        }
    }

    fn cause(&self) -> Option<&Error> { None }
}

impl<'tcx> fmt::Display for EvalError<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            EvalError::PointerOutOfBounds { ptr, access, allocation_size } => {
                write!(f, "{} at offset {}, outside bounds of allocation {} which has size {}",
                       if access { "memory access" } else { "pointer computed" },
                       ptr.offset, ptr.alloc_id, allocation_size)
            },
            EvalError::NoMirFor(ref func) => write!(f, "no mir for `{}`", func),
            EvalError::FunctionPointerTyMismatch(sig, got) =>
                write!(f, "tried to call a function with sig {} through a function pointer of type {}", sig, got),
            EvalError::ArrayIndexOutOfBounds(span, len, index) =>
                write!(f, "index out of bounds: the len is {} but the index is {} at {:?}", len, index, span),
            EvalError::Math(span, ref err) =>
                write!(f, "{:?} at {:?}", err, span),
            EvalError::InvalidChar(c) =>
                write!(f, "tried to interpret an invalid 32-bit value as a char: {}", c),
            EvalError::OutOfMemory { allocation_size, memory_size, memory_usage } =>
                write!(f, "tried to allocate {} more bytes, but only {} bytes are free of the {} byte memory",
                       allocation_size, memory_size - memory_usage, memory_size),
            EvalError::AlignmentCheckFailed { required, has } =>
               write!(f, "tried to access memory with alignment {}, but alignment {} is required",
                      has, required),
            EvalError::TypeNotPrimitive(ty) =>
                write!(f, "expected primitive type, got {}", ty),
            EvalError::Layout(ref err) =>
                write!(f, "rustc layout computation failed: {:?}", err),
            _ => write!(f, "{}", self.description()),
        }
    }
}
