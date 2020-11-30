use crate::spec::{LinkerFlavor, MergeFunctions, PanicStrategy, TargetOptions};
use crate::{abi::Endian, spec::abi::Abi};

pub fn opts(endian: Endian) -> TargetOptions {
    TargetOptions {
        endian,
        linker_flavor: LinkerFlavor::BpfLinker,
        atomic_cas: false,
        executables: true,
        dynamic_linking: true,
        no_builtins: true,
        panic_strategy: PanicStrategy::Abort,
        position_independent_executables: true,
        merge_functions: MergeFunctions::Disabled,
        obj_is_bitcode: true,
        requires_lto: false,
        singlethread: true,
        max_atomic_width: Some(64),
        unsupported_abis: vec![
            Abi::Cdecl,
            Abi::Stdcall { unwind: false },
            Abi::Stdcall { unwind: true },
            Abi::Fastcall,
            Abi::Vectorcall,
            Abi::Thiscall { unwind: false },
            Abi::Thiscall { unwind: true },
            Abi::Aapcs,
            Abi::Win64,
            Abi::SysV64,
            Abi::PtxKernel,
            Abi::Msp430Interrupt,
            Abi::X86Interrupt,
            Abi::AmdGpuKernel,
        ],
        ..Default::default()
    }
}
