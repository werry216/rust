use super::{InlineAsmArch, InlineAsmType};
use rustc_macros::HashStable_Generic;
use std::fmt;

def_reg_class! {
    AArch64 AArch64InlineAsmRegClass {
        reg,
        vreg,
        vreg_low16,
    }
}

impl AArch64InlineAsmRegClass {
    pub fn valid_modifiers(self, _arch: super::InlineAsmArch) -> &'static [char] {
        match self {
            Self::reg => &['w', 'x'],
            Self::vreg | Self::vreg_low16 => &['b', 'h', 's', 'd', 'q', 'v'],
        }
    }

    pub fn suggest_modifier(
        self,
        _arch: InlineAsmArch,
        ty: InlineAsmType,
    ) -> Option<(char, &'static str, Option<&'static str>)> {
        match self {
            Self::reg => {
                if ty.size().bits() <= 32 {
                    Some(('w', "w0", None))
                } else {
                    None
                }
            }
            Self::vreg | Self::vreg_low16 => match ty.size().bits() {
                8 => Some(('b', "b0", None)),
                16 => Some(('h', "h0", None)),
                32 => Some(('s', "s0", None)),
                64 => Some(('d', "d0", None)),
                128 => Some(('q', "q0", None)),
                _ => None,
            },
        }
    }

    pub fn default_modifier(self, _arch: InlineAsmArch) -> Option<(char, &'static str)> {
        match self {
            Self::reg => Some(('x', "x0")),
            Self::vreg | Self::vreg_low16 => Some(('v', "v0")),
        }
    }

    pub fn supported_types(
        self,
        _arch: InlineAsmArch,
    ) -> &'static [(InlineAsmType, Option<&'static str>)] {
        match self {
            Self::reg => types! { _: I8, I16, I32, I64, F32, F64; },
            Self::vreg | Self::vreg_low16 => types! {
                "fp": I8, I16, I32, I64, F32, F64,
                VecI8(8), VecI16(4), VecI32(2), VecI64(1), VecF32(2), VecF64(1),
                VecI8(16), VecI16(8), VecI32(4), VecI64(2), VecF32(4), VecF64(2);
            },
        }
    }
}

def_regs! {
    AArch64 AArch64InlineAsmReg AArch64InlineAsmRegClass {
        x0: reg = ["x0", "w0"],
        x1: reg = ["x1", "w1"],
        x2: reg = ["x2", "w2"],
        x3: reg = ["x3", "w3"],
        x4: reg = ["x4", "w4"],
        x5: reg = ["x5", "w5"],
        x6: reg = ["x6", "w6"],
        x7: reg = ["x7", "w7"],
        x8: reg = ["x8", "w8"],
        x9: reg = ["x9", "w9"],
        x10: reg = ["x10", "w10"],
        x11: reg = ["x11", "w11"],
        x12: reg = ["x12", "w12"],
        x13: reg = ["x13", "w13"],
        x14: reg = ["x14", "w14"],
        x15: reg = ["x15", "w15"],
        x16: reg = ["x16", "w16"],
        x17: reg = ["x17", "w17"],
        x18: reg = ["x18", "w18"],
        x19: reg = ["x19", "w19"],
        x20: reg = ["x20", "w20"],
        x21: reg = ["x21", "w21"],
        x22: reg = ["x22", "w22"],
        x23: reg = ["x23", "w23"],
        x24: reg = ["x24", "w24"],
        x25: reg = ["x25", "w25"],
        x26: reg = ["x26", "w26"],
        x27: reg = ["x27", "w27"],
        x28: reg = ["x28", "w28"],
        x30: reg = ["x30", "w30", "lr"],
        v0: vreg, vreg_low16 = ["v0", "b0", "h0", "s0", "d0", "q0"],
        v1: vreg, vreg_low16 = ["v1", "b1", "h1", "s1", "d1", "q1"],
        v2: vreg, vreg_low16 = ["v2", "b2", "h2", "s2", "d2", "q2"],
        v3: vreg, vreg_low16 = ["v3", "b3", "h3", "s3", "d3", "q3"],
        v4: vreg, vreg_low16 = ["v4", "b4", "h4", "s4", "d4", "q4"],
        v5: vreg, vreg_low16 = ["v5", "b5", "h5", "s5", "d5", "q5"],
        v6: vreg, vreg_low16 = ["v6", "b6", "h6", "s6", "d6", "q6"],
        v7: vreg, vreg_low16 = ["v7", "b7", "h7", "s7", "d7", "q7"],
        v8: vreg, vreg_low16 = ["v8", "b8", "h8", "s8", "d8", "q8"],
        v9: vreg, vreg_low16 = ["v9", "b9", "h9", "s9", "d9", "q9"],
        v10: vreg, vreg_low16 = ["v10", "b10", "h10", "s10", "d10", "q10"],
        v11: vreg, vreg_low16 = ["v11", "b11", "h11", "s11", "d11", "q11"],
        v12: vreg, vreg_low16 = ["v12", "b12", "h12", "s12", "d12", "q12"],
        v13: vreg, vreg_low16 = ["v13", "b13", "h13", "s13", "d13", "q13"],
        v14: vreg, vreg_low16 = ["v14", "b14", "h14", "s14", "d14", "q14"],
        v15: vreg, vreg_low16 = ["v15", "b15", "h15", "s15", "d15", "q15"],
        v16: vreg = ["v16", "b16", "h16", "s16", "d16", "q16"],
        v17: vreg = ["v17", "b17", "h17", "s17", "d17", "q17"],
        v18: vreg = ["v18", "b18", "h18", "s18", "d18", "q18"],
        v19: vreg = ["v19", "b19", "h19", "s19", "d19", "q19"],
        v20: vreg = ["v20", "b20", "h20", "s20", "d20", "q20"],
        v21: vreg = ["v21", "b21", "h21", "s21", "d21", "q21"],
        v22: vreg = ["v22", "b22", "h22", "s22", "d22", "q22"],
        v23: vreg = ["v23", "b23", "h23", "s23", "d23", "q23"],
        v24: vreg = ["v24", "b24", "h24", "s24", "d24", "q24"],
        v25: vreg = ["v25", "b25", "h25", "s25", "d25", "q25"],
        v26: vreg = ["v26", "b26", "h26", "s26", "d26", "q26"],
        v27: vreg = ["v27", "b27", "h27", "s27", "d27", "q27"],
        v28: vreg = ["v28", "b28", "h28", "s28", "d28", "q28"],
        v29: vreg = ["v29", "b29", "h29", "s29", "d29", "q29"],
        v30: vreg = ["v30", "b30", "h30", "s30", "d30", "q30"],
        v31: vreg = ["v31", "b31", "h31", "s31", "d31", "q31"],
        "the frame pointer cannot be used as an operand for inline asm" =
            ["x29", "fp"],
        "the stack pointer cannot be used as an operand for inline asm" =
            ["sp", "wsp"],
        "the zero register cannot be used as an operand for inline asm" =
            ["xzr", "wzr"],
    }
}

impl AArch64InlineAsmReg {
    pub fn emit(
        self,
        out: &mut dyn fmt::Write,
        _arch: InlineAsmArch,
        modifier: Option<char>,
    ) -> fmt::Result {
        let (prefix, index) = if (self as u32) < Self::v0 as u32 {
            (modifier.unwrap_or('x'), self as u32 - Self::x0 as u32)
        } else {
            (modifier.unwrap_or('v'), self as u32 - Self::v0 as u32)
        };
        assert!(index < 32);
        write!(out, "{}{}", prefix, index)
    }
}
