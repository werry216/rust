// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// DO NOT EDIT: autogenerated by etc/platform-intrinsics/generator.py
// ignore-tidy-linelength

#![allow(unused_imports)]

use {Intrinsic, Type};
use IntrinsicDef::Named;

// The default inlining settings trigger a pathological behaviour in
// LLVM, which causes makes compilation very slow. See #28273.
#[inline(never)]
pub fn find(name: &str) -> Option<Intrinsic> {
    if !name.starts_with("powerpc") { return None }
    Some(match &name["powerpc".len()..] {
        "_vec_perm" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 3] = [&::I32x4, &::I32x4, &::I8x16]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vperm")
        },
        "_vec_mradds" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 3] = [&::I16x8, &::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vmhraddshs")
        },
        "_vec_cmpb" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::F32x4, &::F32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vcmpbfp")
        },
        "_vec_cmpeqb" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I8x16, &::I8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vcmpequb")
        },
        "_vec_cmpeqh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vcmpequh")
        },
        "_vec_cmpeqw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I32x4, &::I32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vcmpequw")
        },
        "_vec_cmpgtub" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U8x16, &::U8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vcmpgtub")
        },
        "_vec_cmpgtuh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U16x8, &::U16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vcmpgtuh")
        },
        "_vec_cmpgtuw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U32x4, &::U32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vcmpgtuw")
        },
        "_vec_cmpgtsb" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I8x16, &::I8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vcmpgtsb")
        },
        "_vec_cmpgtsh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vcmpgtsh")
        },
        "_vec_cmpgtsw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I32x4, &::I32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vcmpgtsw")
        },
        "_vec_maxsb" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I8x16, &::I8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vmaxsb")
        },
        "_vec_maxub" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U8x16, &::U8x16]; &INPUTS },
            output: &::U8x16,
            definition: Named("llvm.ppc.altivec.vmaxub")
        },
        "_vec_maxsh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vmaxsh")
        },
        "_vec_maxuh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U16x8, &::U16x8]; &INPUTS },
            output: &::U16x8,
            definition: Named("llvm.ppc.altivec.vmaxuh")
        },
        "_vec_maxsw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I32x4, &::I32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vmaxsw")
        },
        "_vec_maxuw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U32x4, &::U32x4]; &INPUTS },
            output: &::U32x4,
            definition: Named("llvm.ppc.altivec.vmaxuw")
        },
        "_vec_minsb" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I8x16, &::I8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vminsb")
        },
        "_vec_minub" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U8x16, &::U8x16]; &INPUTS },
            output: &::U8x16,
            definition: Named("llvm.ppc.altivec.vminub")
        },
        "_vec_minsh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vminsh")
        },
        "_vec_minuh" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U16x8, &::U16x8]; &INPUTS },
            output: &::U16x8,
            definition: Named("llvm.ppc.altivec.vminuh")
        },
        "_vec_minsw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I32x4, &::I32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vminsw")
        },
        "_vec_minuw" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U32x4, &::U32x4]; &INPUTS },
            output: &::U32x4,
            definition: Named("llvm.ppc.altivec.vminuw")
        },
        "_vec_subsbs" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I8x16, &::I8x16]; &INPUTS },
            output: &::I8x16,
            definition: Named("llvm.ppc.altivec.vsubsbs")
        },
        "_vec_sububs" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U8x16, &::U8x16]; &INPUTS },
            output: &::U8x16,
            definition: Named("llvm.ppc.altivec.vsububs")
        },
        "_vec_subshs" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I16x8, &::I16x8]; &INPUTS },
            output: &::I16x8,
            definition: Named("llvm.ppc.altivec.vsubshs")
        },
        "_vec_subuhs" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U16x8, &::U16x8]; &INPUTS },
            output: &::U16x8,
            definition: Named("llvm.ppc.altivec.vsubuhs")
        },
        "_vec_subsws" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::I32x4, &::I32x4]; &INPUTS },
            output: &::I32x4,
            definition: Named("llvm.ppc.altivec.vsubsws")
        },
        "_vec_subuws" => Intrinsic {
            inputs: { static INPUTS: [&'static Type; 2] = [&::U32x4, &::U32x4]; &INPUTS },
            output: &::U32x4,
            definition: Named("llvm.ppc.altivec.vsubuws")
        },
        _ => return None,
    })
}
